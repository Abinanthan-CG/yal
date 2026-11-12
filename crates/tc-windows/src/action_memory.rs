//! What YAL remembers about the windows it has moved.
//!
//! Upstream remembers, per window: where the window was before it was first
//! touched, where it was left after the last write, which command moved it,
//! and how far through its cycle that command was. From that it answers two
//! questions on every press: which step of the cycle this press is, and
//! whether Restore has somewhere to go back to.
//!
//! Two rules matter and are easy to get wrong:
//!
//! - **The comparison is against the frame that was observed**, never the one
//!   that was asked for. An application that clamped its window to a smaller
//!   size still counts as untouched, as long as what we read back is the
//!   frame we left it at.
//! - **The tolerance is two points**: sub-pixel drift is still ours, a real
//!   drag is the user's, and a user's drag starts a new chain.
//!
//! The memory is bounded, so a long session over many windows cannot grow it
//! without limit, and it is never written to disk: upstream forgets an app's
//! windows when the app quits, and so does this port.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use crate::command::WindowCommandId;
use crate::geometry::{is_tile_command, Rect};

/// How far a window may drift from the frame it was left at and still count
/// as untouched.
pub const TOLERANCE: f64 = 2.0;

/// How many windows are remembered at most.
pub const DEFAULT_CAPACITY: usize = 64;

/// What the caller knows when it asks the memory to resolve a press.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press {
    /// The command that was pressed, or `None` for a custom size, which
    /// never cycles and is never a tile.
    pub command: Option<WindowCommandId>,
    /// Where the window is now.
    pub current_frame: Rect,
    /// The display the window is on.
    pub current_screen_id: i64,
    /// How many presses the chain has, which is one for a command that does
    /// not cycle and for cycling switched off entirely.
    pub cycle_length: usize,
    /// When the press happened.
    pub now: Instant,
    /// Whether the window is in fullscreen.
    pub is_fullscreen: bool,
}

/// What actually landed, read back from the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Applied {
    /// Where the window ended up.
    pub frame: Rect,
    /// The display it ended up on.
    pub screen_id: i64,
    /// Whether it ended up in fullscreen.
    pub fullscreen: bool,
}

/// What is remembered about one window.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Where the window was before YAL first touched it.
    pub restore_frame: Rect,
    /// Whether Restore should put the window back into fullscreen rather
    /// than place the restore frame.
    pub restores_fullscreen: bool,
    /// Where the window was **observed** after the last write - not what was
    /// asked for.
    pub applied_frame: Rect,
    /// Whether the window was left fullscreen, so its fullscreen frame still
    /// counts as untouched.
    pub applied_fullscreen: bool,
    /// The command that moved it.
    pub command: Option<WindowCommandId>,
    /// The step the next press of that command uses.
    ///
    /// The record holds the step that comes *next* rather than the one that
    /// was applied, so that resetting the cycle is a matter of putting the
    /// half back at the front of it.
    pub next_step: usize,
    /// Where it landed; a press from any other display starts a new chain.
    pub screen_id: i64,
    /// The display the chain started on.
    pub origin_screen_id: i64,
    /// When the last write happened.
    pub at: Instant,
}

/// What a press resolves to.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// The cycle position for this press.
    pub step: usize,
    /// The display the chain started on, which the display cycle counts the
    /// step from.
    pub origin_screen_id: i64,
    /// Where the window goes back to.
    pub restore_frame: Rect,
    /// Whether Restore re-enters fullscreen instead of placing the frame.
    pub restores_fullscreen: bool,
    /// False on the first sight: there is nothing to go back to yet, so
    /// Restore must do nothing.
    pub can_restore: bool,
    /// Set only when the window still sits exactly where a tile command left
    /// it.
    pub last_tile_command: Option<WindowCommandId>,
}

/// What YAL remembers about the windows it has moved.
#[derive(Debug, Clone)]
pub struct WindowActionMemory<K> {
    capacity: usize,
    cycle_timeout: Option<Duration>,
    records: HashMap<K, Record>,
    /// Least recently used first, so the eviction is a removal from the
    /// front.
    order: Vec<K>,
}

impl<K> Default for WindowActionMemory<K> {
    fn default() -> Self {
        WindowActionMemory {
            capacity: DEFAULT_CAPACITY,
            cycle_timeout: None,
            records: HashMap::new(),
            order: Vec::new(),
        }
    }
}

impl<K: Eq + Hash + Clone> WindowActionMemory<K> {
    /// A memory that remembers at most `capacity` windows.
    pub fn new(capacity: usize) -> WindowActionMemory<K> {
        WindowActionMemory {
            capacity,
            ..WindowActionMemory::default()
        }
    }

    /// A memory that forgets a chain after a pause.
    ///
    /// Upstream's setting is off by default; when it is on, a cold cycle
    /// restarts from the half rather than continuing mid-sequence.
    pub fn with_cycle_timeout(
        capacity: usize,
        cycle_timeout: Option<Duration>,
    ) -> WindowActionMemory<K> {
        WindowActionMemory {
            capacity,
            cycle_timeout,
            ..WindowActionMemory::default()
        }
    }

    /// How many windows are remembered.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// What is remembered about one window.
    pub fn record(&self, key: &K) -> Option<&Record> {
        self.records.get(key)
    }

    /// Resolve the cycle step and the restore point for a press.
    ///
    /// The caller writes afterwards with [`commit`](Self::commit), once it
    /// knows what actually landed.
    pub fn decide(&self, key: &K, press: &Press) -> Decision {
        // First sight: capture where it was, so Restore works for a window
        // that was never moved.
        let Some(record) = self.records.get(key) else {
            return Decision {
                step: 0,
                origin_screen_id: press.current_screen_id,
                restore_frame: press.current_frame,
                restores_fullscreen: press.is_fullscreen,
                can_restore: false,
                last_tile_command: None,
            };
        };

        // Against the observed frame, never the requested one.
        let untouched = if press.is_fullscreen {
            record.applied_fullscreen
        } else {
            approximately_equal(press.current_frame, record.applied_frame)
        };
        if !untouched {
            return Decision {
                step: 0,
                origin_screen_id: press.current_screen_id,
                restore_frame: press.current_frame,
                restores_fullscreen: press.is_fullscreen,
                can_restore: true,
                last_tile_command: None,
            };
        }

        let last_tile_command = record.command.filter(|command| is_tile_command(*command));
        let expired = self
            .cycle_timeout
            .map(|timeout| press.now.duration_since(record.at) > timeout)
            .unwrap_or(false);
        // A length of one covers both a command that does not cycle and
        // cycling switched off entirely.
        let continues = press.cycle_length > 1
            && press.command == record.command
            && press.current_screen_id == record.screen_id
            && !expired;
        Decision {
            step: if continues { record.next_step } else { 0 },
            origin_screen_id: if continues {
                record.origin_screen_id
            } else {
                press.current_screen_id
            },
            restore_frame: record.restore_frame,
            restores_fullscreen: record.restores_fullscreen,
            can_restore: true,
            last_tile_command,
        }
    }

    /// Record what actually landed.
    ///
    /// The applied frame must be read back from the window, never assumed.
    pub fn commit(&mut self, key: K, press: &Press, decision: &Decision, applied: &Applied) {
        // The step that comes next, so the next press of the same command
        // continues the chain instead of repeating it.
        let next_step = if press.cycle_length > 1 {
            (decision.step + 1) % press.cycle_length
        } else {
            0
        };
        self.records.insert(
            key.clone(),
            Record {
                restore_frame: decision.restore_frame,
                restores_fullscreen: decision.restores_fullscreen,
                applied_frame: applied.frame,
                applied_fullscreen: applied.fullscreen,
                command: press.command,
                next_step,
                screen_id: applied.screen_id,
                origin_screen_id: decision.origin_screen_id,
                at: press.now,
            },
        );
        self.touch(key);
    }

    /// Break the cycle chain while keeping the restore point.
    ///
    /// This is what fullscreen needs: it resets the cycle but keeps the
    /// Restore point.
    pub fn forget_cycle(&mut self, key: &K, is_fullscreen: bool) {
        let Some(mut record) = self.records.get(key).cloned() else {
            return;
        };
        record.next_step = 0;
        record.origin_screen_id = record.screen_id;
        record.applied_fullscreen = is_fullscreen;
        self.records.insert(key.clone(), record);
    }

    /// Forget every window whose key matches.
    ///
    /// Upstream uses this when an app quits: the windows of that app are
    /// forgotten, and the memory of them with them.
    pub fn forget_where(&mut self, predicate: impl Fn(&K) -> bool) {
        let doomed: Vec<K> = self
            .records
            .keys()
            .filter(|key| predicate(key))
            .cloned()
            .collect();
        if doomed.is_empty() {
            return;
        }
        for key in &doomed {
            self.records.remove(key);
        }
        self.order.retain(|key| !doomed.contains(key));
    }

    /// Forget one window.
    pub fn forget(&mut self, key: &K) {
        if self.records.remove(key).is_none() {
            return;
        }
        self.order.retain(|held| held != key);
    }

    /// Mark a key as the most recently used, and evict the oldest when the
    /// memory is full.
    fn touch(&mut self, key: K) {
        self.order.retain(|held| *held != key);
        self.order.push(key);
        while self.order.len() > self.capacity {
            let Some(oldest) = self.order.first().cloned() else {
                break;
            };
            self.order.remove(0);
            self.records.remove(&oldest);
        }
    }
}

/// Whether two frames are the same, within the tolerance.
fn approximately_equal(a: Rect, b: Rect) -> bool {
    (a.min_x() - b.min_x()).abs() <= TOLERANCE
        && (a.min_y() - b.min_y()).abs() <= TOLERANCE
        && (a.width() - b.width()).abs() <= TOLERANCE
        && (a.height() - b.height()).abs() <= TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: f64, y: f64) -> Rect {
        Rect::new(x, y, 400.0, 300.0)
    }

    fn memory() -> WindowActionMemory<u32> {
        WindowActionMemory::new(64)
    }

    /// A press, for the tests.
    fn press(
        command: WindowCommandId,
        current_frame: Rect,
        screen: i64,
        cycle_length: usize,
        now: Instant,
    ) -> Press {
        Press {
            command: Some(command),
            current_frame,
            current_screen_id: screen,
            cycle_length,
            now,
            is_fullscreen: false,
        }
    }

    /// What landed, for the tests.
    fn applied(frame: Rect, screen: i64) -> Applied {
        Applied {
            frame,
            screen_id: screen,
            fullscreen: false,
        }
    }

    #[test]
    fn the_first_sight_has_nothing_to_restore() {
        let memory = memory();
        let now = Instant::now();
        let decision = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        assert_eq!(decision.step, 0);
        assert!(!decision.can_restore);
        assert_eq!(decision.restore_frame, frame(10.0, 20.0));
        assert_eq!(decision.origin_screen_id, 7);
    }

    #[test]
    fn a_second_press_of_the_same_command_steps_the_cycle() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let second = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(0.0, 0.0), 7, 3, now),
        );
        assert_eq!(second.step, 1);
        assert!(second.can_restore);
        assert_eq!(
            second.restore_frame,
            frame(10.0, 20.0),
            "the starting frame"
        );
    }

    #[test]
    fn the_cycle_wraps_around() {
        let mut memory = memory();
        let now = Instant::now();
        let mut press = press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 2, now);
        let mut decision = memory.decide(&1, &press);
        memory.commit(1, &press, &decision, &applied(frame(0.0, 0.0), 7));
        press.current_frame = frame(0.0, 0.0);
        decision = memory.decide(&1, &press);
        assert_eq!(decision.step, 1);
        memory.commit(1, &press, &decision, &applied(frame(0.0, 0.0), 7));
        decision = memory.decide(&1, &press);
        assert_eq!(decision.step, 0, "the chain wraps");
    }

    #[test]
    fn a_window_the_user_moved_starts_a_new_chain() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        // The window was dragged: it no longer sits where it was left.
        let dragged = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(300.0, 200.0), 7, 3, now),
        );
        assert_eq!(dragged.step, 0);
        assert_eq!(dragged.restore_frame, frame(300.0, 200.0), "the new start");
    }

    #[test]
    fn a_drift_of_two_points_is_still_untouched() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let drifted = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(2.0, 0.0), 7, 3, now),
        );
        assert_eq!(drifted.step, 1, "two points of drift is still ours");
    }

    #[test]
    fn a_drag_of_three_points_is_the_users_own() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let dragged = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(3.0, 0.0), 7, 3, now),
        );
        assert_eq!(dragged.step, 0, "three points is a real drag");
    }

    #[test]
    fn a_different_command_starts_a_new_chain() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let other = memory.decide(
            &1,
            &press(WindowCommandId::RightHalf, frame(0.0, 0.0), 7, 3, now),
        );
        assert_eq!(other.step, 0);
    }

    #[test]
    fn a_press_on_another_display_starts_a_new_chain() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let moved = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(0.0, 0.0), 9, 3, now),
        );
        assert_eq!(moved.step, 0);
        assert_eq!(
            moved.origin_screen_id, 9,
            "the chain starts on the new display"
        );
    }

    #[test]
    fn waiting_between_presses_does_not_reset_the_chain() {
        // The chain timeout is off by default, so a pause changes nothing.
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let later = memory.decide(
            &1,
            &press(
                WindowCommandId::LeftHalf,
                frame(0.0, 0.0),
                7,
                3,
                now + Duration::from_secs(3600),
            ),
        );
        assert_eq!(later.step, 1, "the chain carried on");
    }

    #[test]
    fn a_pause_forgets_the_chain_but_keeps_the_restore_point() {
        let mut memory = WindowActionMemory::with_cycle_timeout(64, Some(Duration::from_secs(30)));
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let later = memory.decide(
            &1,
            &press(
                WindowCommandId::LeftHalf,
                frame(0.0, 0.0),
                7,
                3,
                now + Duration::from_secs(60),
            ),
        );
        assert_eq!(later.step, 0, "the chain started over");
        assert_eq!(
            later.restore_frame,
            frame(10.0, 20.0),
            "the restore point stayed"
        );
    }

    #[test]
    fn a_non_cycling_command_never_steps() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::Maximize, frame(10.0, 20.0), 7, 1, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::Maximize, frame(10.0, 20.0), 7, 1, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let second = memory.decide(
            &1,
            &press(WindowCommandId::Maximize, frame(0.0, 0.0), 7, 1, now),
        );
        assert_eq!(second.step, 0);
    }

    #[test]
    fn the_last_tile_is_remembered_only_when_the_window_is_where_it_was_left() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(
                WindowCommandId::TopLeftQuarter,
                frame(10.0, 20.0),
                7,
                1,
                now,
            ),
        );
        memory.commit(
            1,
            &press(
                WindowCommandId::TopLeftQuarter,
                frame(10.0, 20.0),
                7,
                1,
                now,
            ),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let second = memory.decide(
            &1,
            &press(WindowCommandId::Maximize, frame(0.0, 0.0), 7, 1, now),
        );
        assert_eq!(
            second.last_tile_command,
            Some(WindowCommandId::TopLeftQuarter)
        );
    }

    #[test]
    fn a_command_that_is_not_a_tile_is_never_remembered_as_one() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::Maximize, frame(10.0, 20.0), 7, 1, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::Maximize, frame(10.0, 20.0), 7, 1, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        let second = memory.decide(
            &1,
            &press(WindowCommandId::Maximize, frame(0.0, 0.0), 7, 1, now),
        );
        assert_eq!(second.last_tile_command, None);
    }

    #[test]
    fn the_memory_is_bounded_and_forgets_the_oldest() {
        let mut memory = WindowActionMemory::new(2);
        let now = Instant::now();
        for key in 1..=3u32 {
            let press = press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 1, now);
            let decision = memory.decide(&key, &press);
            memory.commit(key, &press, &decision, &applied(frame(0.0, 0.0), 7));
        }
        assert_eq!(memory.count(), 2);
        assert!(memory.record(&1).is_none(), "the oldest was evicted");
        assert!(memory.record(&3).is_some());
    }

    #[test]
    fn a_window_that_is_used_again_is_not_evicted_first() {
        let mut memory = WindowActionMemory::new(2);
        let now = Instant::now();
        // Three windows are touched, but the first one is touched again in
        // between, so the second is the oldest and is the one that goes.
        for key in [1u32, 2, 1, 3] {
            let press = press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 1, now);
            let decision = memory.decide(&key, &press);
            memory.commit(key, &press, &decision, &applied(frame(0.0, 0.0), 7));
        }
        assert_eq!(memory.count(), 2);
        assert!(memory.record(&1).is_some(), "it was used last");
        assert!(memory.record(&2).is_none(), "the other one was evicted");
        assert!(memory.record(&3).is_some());
    }

    #[test]
    fn forgetting_the_cycle_keeps_the_restore_point() {
        let mut memory = memory();
        let now = Instant::now();
        let first = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
        );
        memory.commit(
            1,
            &press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 3, now),
            &first,
            &applied(frame(0.0, 0.0), 7),
        );
        memory.forget_cycle(&1, false);
        let after = memory.decide(
            &1,
            &press(WindowCommandId::LeftHalf, frame(0.0, 0.0), 7, 3, now),
        );
        assert_eq!(after.step, 0, "the chain started over");
        assert_eq!(
            after.restore_frame,
            frame(10.0, 20.0),
            "the restore point stayed"
        );
    }

    #[test]
    fn the_windows_of_an_app_that_quits_are_forgotten() {
        let mut memory = memory();
        let now = Instant::now();
        for key in [1u32, 2] {
            let press = press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 1, now);
            let decision = memory.decide(&key, &press);
            memory.commit(key, &press, &decision, &applied(frame(0.0, 0.0), 7));
        }
        memory.forget_where(|key| *key == 1);
        assert_eq!(memory.count(), 1);
        assert!(memory.record(&1).is_none());
        assert!(memory.record(&2).is_some());
    }

    #[test]
    fn one_window_can_be_forgotten() {
        let mut memory = memory();
        let now = Instant::now();
        let press = press(WindowCommandId::LeftHalf, frame(10.0, 20.0), 7, 1, now);
        let decision = memory.decide(&1, &press);
        memory.commit(1, &press, &decision, &applied(frame(0.0, 0.0), 7));
        memory.forget(&1);
        assert_eq!(memory.count(), 0);
    }
}
