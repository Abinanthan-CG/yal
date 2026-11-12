//! The placement geometry.
//!
//! Pure arithmetic: given a rectangle, a set of displays and a gap, where
//! does a window go when one of the thirty-nine commands runs? No Win32, no
//! desktop, no window - so every rule can be tested on a machine with no
//! screen at all.
//!
//! The rules are upstream's, ported from `WindowPlacementEngine.swift`. The
//! shape of the port is the shape of the donor project in
//! `C:\Users\dabin\development\yal` (GPL-3.0-only): one pure function that
//! answers a rectangle, and the platform calls kept out of it.
//!
//! **Coordinates.** The engine works in the display's own space with the
//! origin at the top left and +Y pointing down, which is what Windows already
//! uses and what macOS's accessibility space uses too. The caller supplies the
//! work area in the unit it wants the answer in; on Windows that is physical
//! pixels, and the gap - which upstream counts in points - has to be scaled
//! by the display's scale before it is handed over.

use crate::command::WindowCommandId;

/// A rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// The left edge.
    pub x: f64,
    /// The top edge.
    pub y: f64,
    /// The width.
    pub width: f64,
    /// The height.
    pub height: f64,
}

impl Rect {
    /// A rectangle from its edges.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// The left edge.
    pub fn min_x(self) -> f64 {
        self.x
    }

    /// The top edge.
    pub fn min_y(self) -> f64 {
        self.y
    }

    /// The right edge.
    pub fn max_x(self) -> f64 {
        self.x + self.width
    }

    /// The bottom edge.
    pub fn max_y(self) -> f64 {
        self.y + self.height
    }

    /// The width.
    pub fn width(self) -> f64 {
        self.width
    }

    /// The height.
    pub fn height(self) -> f64 {
        self.height
    }

    /// The horizontal middle.
    pub fn mid_x(self) -> f64 {
        self.x + self.width / 2.0
    }

    /// The vertical middle.
    pub fn mid_y(self) -> f64 {
        self.y + self.height / 2.0
    }

    /// Whether the rectangle has no area.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// The area shared with another rectangle, or `None` when they do not meet.
    pub fn intersection(self, other: Rect) -> Option<Rect> {
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        if max_x <= min_x || max_y <= min_y {
            return None;
        }
        Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }

    /// Whether a point is inside the rectangle.
    pub fn contains_point(self, x: f64, y: f64) -> bool {
        x >= self.min_x() && x < self.max_x() && y >= self.min_y() && y < self.max_y()
    }
}

/// A display.
///
/// The frame is the whole display; the visible frame is the part a window may
/// use, which on Windows is the work area - the display minus the taskbar and
/// any other app bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// The display's id. Any stable number works; Windows uses the device id.
    pub id: i64,
    /// The whole display.
    pub frame: Rect,
    /// The part of the display a window may use.
    pub visible_frame: Rect,
}

/// The axis a window is anchored on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// The leading edge: left, or top.
    Min,
    /// The middle.
    Center,
    /// The trailing edge: right, or bottom.
    Max,
}

/// Where a window that refused to shrink sits in its slot.
///
/// A left half stays left-aligned even when the window is wider than the slot,
/// so an application that refuses to shrink does not end up in the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// The horizontal anchor.
    pub horizontal: Axis,
    /// The vertical anchor.
    pub vertical: Axis,
}

impl Anchor {
    /// The top-left anchor.
    pub fn top_leading() -> Anchor {
        Anchor {
            horizontal: Axis::Min,
            vertical: Axis::Min,
        }
    }

    /// The centred anchor.
    pub fn centered() -> Anchor {
        Anchor {
            horizontal: Axis::Center,
            vertical: Axis::Center,
        }
    }

    /// Places a size inside a slot, according to the anchor.
    pub fn place(self, size: (f64, f64), slot: Rect) -> Rect {
        let origin = |axis: Axis, slot_min: f64, slot_length: f64, length: f64| match axis {
            Axis::Min => slot_min,
            Axis::Center => slot_min + (slot_length - length) / 2.0,
            Axis::Max => slot_min + slot_length - length,
        };
        Rect::new(
            origin(self.horizontal, slot.min_x(), slot.width(), size.0),
            origin(self.vertical, slot.min_y(), slot.height(), size.1),
            size.0,
            size.1,
        )
    }
}

/// What a repeat press of a half does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCycle {
    /// Repeating a half keeps the same frame.
    Off,
    /// Repeating a half steps through ½, ⅓ and ⅔.
    Sizes,
    /// Repeating a half moves it along the strip of half-slots every display
    /// contributes.
    Displays,
}

impl WindowCycle {
    /// The name the setting is shown under.
    pub fn title(self) -> &'static str {
        match self {
            WindowCycle::Off => "None",
            WindowCycle::Sizes => "Cycle ½, ⅓ and ⅔",
            WindowCycle::Displays => "Cycle displays",
        }
    }

    /// The line under the setting's name.
    pub fn detail(self) -> &'static str {
        match self {
            WindowCycle::Off => "Repeating a half keeps the same frame.",
            WindowCycle::Sizes => "Repeating a half steps through ⅓ and ⅔.",
            WindowCycle::Displays => "Repeating a half moves it across your displays.",
        }
    }

    /// The stored id.
    pub fn as_str(self) -> &'static str {
        match self {
            WindowCycle::Off => "off",
            WindowCycle::Sizes => "sizes",
            WindowCycle::Displays => "displays",
        }
    }

    /// Look a cycle mode up by its stored id.
    pub fn parse(value: &str) -> Option<WindowCycle> {
        match value {
            "off" => Some(WindowCycle::Off),
            "sizes" => Some(WindowCycle::Sizes),
            "displays" => Some(WindowCycle::Displays),
            _ => None,
        }
    }
}

/// Everything the geometry needs to answer a command.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementInput {
    /// The command that ran.
    pub command: WindowCommandId,
    /// Where the window is now.
    pub window_frame: Rect,
    /// The displays the machine has.
    pub screens: Vec<Screen>,
    /// The gap between windows, in the same unit as the rectangles.
    pub gap: f64,
    /// Which press of the cycle this is.
    pub step: usize,
    /// The cycling mode.
    pub cycle: WindowCycle,
    /// The display the chain started on.
    pub origin_screen_id: Option<i64>,
    /// The frame to go back to, for the restore command.
    pub restore_frame: Option<Rect>,
    /// The tile that last placed this window, so a display move re-derives
    /// rather than scales.
    pub last_tile_command: Option<WindowCommandId>,
    /// Whether a display move wraps around the strip.
    pub wraps_displays: bool,
    /// Whether a display move keeps the window's proportions.
    pub keeps_proportions: bool,
}

impl PlacementInput {
    /// The common case: one command, one window, the machine's displays and
    /// no gap.
    pub fn new(
        command: WindowCommandId,
        window_frame: Rect,
        screens: Vec<Screen>,
        gap: f64,
    ) -> PlacementInput {
        PlacementInput {
            command,
            window_frame,
            screens,
            gap,
            step: 0,
            cycle: WindowCycle::Off,
            origin_screen_id: None,
            restore_frame: None,
            last_tile_command: None,
            wraps_displays: true,
            keeps_proportions: true,
        }
    }
}

/// Where a window goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// The rectangle the window should take.
    pub frame: Rect,
    /// The display it should land on.
    pub screen_id: i64,
    /// How to place the window in its slot if it refuses to shrink.
    pub anchor: Anchor,
    /// Whether the window's size should be written as well as its position.
    pub resizes: bool,
}

/// Make Larger and Make Smaller, and the four nudges, all step by this
/// fraction of the screen.
const STEP_FRACTION: f64 = 0.05;
/// Almost Maximize leaves a tenth of the screen around the window.
const ALMOST_MAXIMIZE_FRACTION: f64 = 0.9;
/// Reasonable Size is three fifths of the screen.
const REASONABLE_SIZE_FRACTION: f64 = 0.6;
/// The hard ceiling on Reasonable Size: sixty per cent of a 5K display is not
/// a reasonable size for a window.
const REASONABLE_SIZE_MAX: (f64, f64) = (1025.0, 900.0);

const ONE_THIRD: f64 = 1.0 / 3.0;
const TWO_THIRDS: f64 = 2.0 / 3.0;

/// The widths the size cycle steps through.
const SIZE_CYCLE: [f64; 3] = [0.5, ONE_THIRD, TWO_THIRDS];

/// The smallest gap the Settings pane offers, in points.
pub const GAP_MIN: f64 = 0.0;
/// The largest gap the Settings pane offers, in points.
pub const GAP_MAX: f64 = 64.0;

/// The target placement, or `None` when nothing should be written at all.
///
/// `None` is the honest answer for a command with no window to move (the
/// Space switches), for a machine with no displays, and for a window that
/// reports no position.
pub fn placement(input: &PlacementInput) -> Option<Placement> {
    let command = input.command;
    if command.kind() != crate::command::WindowCommandKind::Geometry
        && command.kind() != crate::command::WindowCommandKind::Restore
    {
        return None;
    }
    if input.screens.is_empty() {
        return None;
    }
    if command.kind() == crate::command::WindowCommandKind::Restore {
        return restore_placement(input);
    }
    let host = screen_containing(input.window_frame, &input.screens)?;
    if host.visible_frame.width() <= 0.0 || host.visible_frame.height() <= 0.0 {
        return None;
    }
    if matches!(
        command,
        WindowCommandId::NextDisplay | WindowCommandId::PreviousDisplay
    ) {
        return display_placement(input, host);
    }

    let gap = sanitized_gap(input.gap, host.visible_frame);
    let step = wrapped(
        input.step as i64,
        cycle_length(command, &input.screens, input.cycle) as i64,
    ) as usize;

    if let Some(half) = Half::of(command) {
        return Some(half_placement(input, half, host, step));
    }
    if command == WindowCommandId::CenterHalf {
        return Some(tile_placement(
            centered_fractions(SIZE_CYCLE[step]),
            host,
            gap,
        ));
    }
    if let Some(fractions) = tile_fractions(command) {
        return Some(tile_placement(fractions, host, gap));
    }

    let canvas = canvas(host.visible_frame, gap);
    if canvas.width() <= 0.0 || canvas.height() <= 0.0 {
        return None;
    }
    let current = input.window_frame;

    match command {
        WindowCommandId::Maximize => Some(Placement {
            frame: canvas,
            screen_id: host.id,
            anchor: Anchor::top_leading(),
            resizes: true,
        }),
        WindowCommandId::AlmostMaximize => {
            let size = (
                canvas.width() * ALMOST_MAXIMIZE_FRACTION,
                canvas.height() * ALMOST_MAXIMIZE_FRACTION,
            );
            Some(Placement {
                frame: rounded(Anchor::centered().place(size, canvas)),
                screen_id: host.id,
                anchor: Anchor::centered(),
                resizes: true,
            })
        }
        WindowCommandId::ReasonableSize => {
            // The cap makes this display-independent; 0.6 < 1 stays inside the
            // canvas unclamped.
            let size = (
                (canvas.width() * REASONABLE_SIZE_FRACTION).min(REASONABLE_SIZE_MAX.0),
                (canvas.height() * REASONABLE_SIZE_FRACTION).min(REASONABLE_SIZE_MAX.1),
            );
            Some(Placement {
                frame: rounded(Anchor::centered().place(size, canvas)),
                screen_id: host.id,
                anchor: Anchor::centered(),
                resizes: true,
            })
        }
        // Both keep the untouched axis, but clamp it, so an off-display
        // window comes back.
        WindowCommandId::MaximizeHeight => {
            let frame = Rect::new(
                current.min_x(),
                canvas.min_y(),
                current.width(),
                canvas.height(),
            );
            Some(Placement {
                frame: rounded(clamped(frame, canvas)),
                screen_id: host.id,
                anchor: Anchor::top_leading(),
                resizes: true,
            })
        }
        WindowCommandId::MaximizeWidth => {
            let frame = Rect::new(
                canvas.min_x(),
                current.min_y(),
                canvas.width(),
                current.height(),
            );
            Some(Placement {
                frame: rounded(clamped(frame, canvas)),
                screen_id: host.id,
                anchor: Anchor::top_leading(),
                resizes: true,
            })
        }
        WindowCommandId::Center => {
            let size = (
                current.width().min(canvas.width()),
                current.height().min(canvas.height()),
            );
            Some(Placement {
                frame: rounded(Anchor::centered().place(size, canvas)),
                screen_id: host.id,
                anchor: Anchor::centered(),
                resizes: true,
            })
        }
        WindowCommandId::MakeLarger | WindowCommandId::MakeSmaller => Some(Placement {
            frame: resized(current, canvas, command == WindowCommandId::MakeLarger),
            screen_id: host.id,
            anchor: Anchor::centered(),
            resizes: true,
        }),
        WindowCommandId::MoveLeft
        | WindowCommandId::MoveRight
        | WindowCommandId::MoveUp
        | WindowCommandId::MoveDown
        | WindowCommandId::MoveToLeftEdge
        | WindowCommandId::MoveToRightEdge
        | WindowCommandId::MoveToTopEdge
        | WindowCommandId::MoveToBottomEdge => {
            if current.is_empty() {
                return None;
            }
            Some(Placement {
                frame: shifted(current, canvas, command),
                screen_id: host.id,
                anchor: Anchor::top_leading(),
                resizes: false,
            })
        }
        _ => None,
    }
}

/// The display a window lives on: the most overlapping area wins, otherwise
/// the one holding its centre.
pub fn screen_containing(frame: Rect, screens: &[Screen]) -> Option<Screen> {
    if screens.is_empty() {
        return None;
    }
    let mut best: Option<(Screen, f64)> = None;
    for screen in screens {
        let area = screen
            .frame
            .intersection(frame)
            .map(|overlap| overlap.width() * overlap.height())
            .unwrap_or(0.0);
        if area > best.map(|(_, area)| area).unwrap_or(0.0) {
            best = Some((*screen, area));
        }
    }
    if let Some((screen, area)) = best {
        if area > 0.0 {
            return Some(screen);
        }
    }
    screens
        .iter()
        .find(|screen| screen.frame.contains_point(frame.mid_x(), frame.mid_y()))
        .copied()
        .or_else(|| screens.first().copied())
}

/// The displays in a stable order: left to right, then top to bottom.
///
/// Windows hands the displays over in no particular order, so the strip a
/// cycle walks has to be ordered by hand.
pub fn ordered_screens(screens: &[Screen]) -> Vec<Screen> {
    let mut ordered = screens.to_vec();
    ordered.sort_by(|a, b| {
        a.frame
            .min_x()
            .partial_cmp(&b.frame.min_x())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.frame
                    .min_y()
                    .partial_cmp(&b.frame.min_y())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    ordered
}

fn display_placement(input: &PlacementInput, host: Screen) -> Option<Placement> {
    let ordered = ordered_screens(&input.screens);
    // A single display makes both commands a quiet no-op rather than a
    // pointless re-place.
    if ordered.len() <= 1 {
        return None;
    }
    let index = ordered.iter().position(|screen| screen.id == host.id)?;
    let next = if input.command == WindowCommandId::NextDisplay {
        index as i64 + 1
    } else {
        index as i64 - 1
    };
    if !input.wraps_displays && !(0..ordered.len() as i64).contains(&next) {
        return None;
    }
    let destination = ordered[((next + ordered.len() as i64) % ordered.len() as i64) as usize];
    let frame = moved(input, host, destination);
    Some(Placement {
        frame,
        screen_id: destination.id,
        anchor: Anchor::centered(),
        resizes: true,
    })
}

fn moved(input: &PlacementInput, from: Screen, to: Screen) -> Rect {
    let frame = input.window_frame;
    let target_gap = sanitized_gap(input.gap, to.visible_frame);
    let source = canvas(
        from.visible_frame,
        sanitized_gap(input.gap, from.visible_frame),
    );
    let target = canvas(to.visible_frame, target_gap);
    if !input.keeps_proportions {
        let kept = Rect::new(
            target.min_x() + frame.min_x() - source.min_x(),
            target.min_y() + frame.min_y() - source.min_y(),
            to.visible_frame.width().min(frame.width()),
            to.visible_frame.height().min(frame.height()),
        );
        return rounded(clamped(kept, to.visible_frame));
    }
    // Exactness beats proportion: an untouched tile re-derives instead of
    // being scaled.
    if let Some(last_tile) = input.last_tile_command {
        if let Some(fractions) = tile_fractions(last_tile) {
            return tile(
                to.visible_frame,
                fractions.x0,
                fractions.x1,
                fractions.y0,
                fractions.y1,
                target_gap,
            );
        }
    }
    if source.width() <= 0.0 || source.height() <= 0.0 {
        return frame;
    }
    let relative_x = (frame.min_x() - source.min_x()) / source.width();
    let relative_y = (frame.min_y() - source.min_y()) / source.height();
    let scaled = Rect::new(
        target.min_x() + relative_x * target.width(),
        target.min_y() + relative_y * target.height(),
        to.visible_frame
            .width()
            .min(frame.width() / source.width() * target.width()),
        to.visible_frame
            .height()
            .min(frame.height() / source.height() * target.height()),
    );
    rounded(clamped(scaled, to.visible_frame))
}

fn restore_placement(input: &PlacementInput) -> Option<Placement> {
    let restore_frame = input.restore_frame?;
    let host = screen_containing(restore_frame, &input.screens)?;
    let overlap = host.visible_frame.intersection(restore_frame);
    // A restore point stranded off every display comes back centred, not
    // unreachable.
    let stranded = match overlap {
        None => true,
        Some(overlap) => overlap.width() < 40.0 || overlap.height() < 40.0,
    };
    let frame = if stranded {
        rounded(clamped(
            Anchor::centered().place(
                (restore_frame.width(), restore_frame.height()),
                host.visible_frame,
            ),
            host.visible_frame,
        ))
    } else {
        restore_frame
    };
    Some(Placement {
        frame,
        screen_id: host.id,
        anchor: Anchor::centered(),
        resizes: true,
    })
}

/// A tile's fractional bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fractions {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    anchor: Anchor,
}

/// A half: an axis and the edge it is pinned to.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Half {
    axis: HalfAxis,
    edge: HalfEdge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HalfAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HalfEdge {
    Leading,
    Trailing,
}

impl Half {
    /// The half a command names, or `None` when it is not one of the four.
    fn of(command: WindowCommandId) -> Option<Half> {
        match command {
            WindowCommandId::LeftHalf => Some(Half {
                axis: HalfAxis::Horizontal,
                edge: HalfEdge::Leading,
            }),
            WindowCommandId::RightHalf => Some(Half {
                axis: HalfAxis::Horizontal,
                edge: HalfEdge::Trailing,
            }),
            WindowCommandId::TopHalf => Some(Half {
                axis: HalfAxis::Vertical,
                edge: HalfEdge::Leading,
            }),
            WindowCommandId::BottomHalf => Some(Half {
                axis: HalfAxis::Vertical,
                edge: HalfEdge::Trailing,
            }),
            _ => None,
        }
    }

    /// Bounds covering a fraction of the screen along the axis, pinned to the
    /// edge.
    fn fractions(&self, fraction: f64) -> Fractions {
        let leads = self.edge == HalfEdge::Leading;
        let (span0, span1) = if leads {
            (0.0, fraction)
        } else {
            (1.0 - fraction, 1.0)
        };
        let along = if leads { Axis::Min } else { Axis::Max };
        match self.axis {
            HalfAxis::Horizontal => Fractions {
                x0: span0,
                x1: span1,
                y0: 0.0,
                y1: 1.0,
                anchor: Anchor {
                    horizontal: along,
                    vertical: Axis::Min,
                },
            },
            HalfAxis::Vertical => Fractions {
                x0: 0.0,
                x1: 1.0,
                y0: span0,
                y1: span1,
                anchor: Anchor {
                    horizontal: Axis::Min,
                    vertical: along,
                },
            },
        }
    }
}

/// Presses before the cycle wraps; one means the command does not cycle.
pub fn cycle_length(command: WindowCommandId, screens: &[Screen], cycle: WindowCycle) -> usize {
    if !command.cycles_on_repeat() {
        return 1;
    }
    match cycle {
        WindowCycle::Off => 1,
        WindowCycle::Sizes => SIZE_CYCLE.len(),
        // Only an edge half has a strip slot; one display is a no-op, not an
        // in-place flip.
        WindowCycle::Displays => {
            if screens.len() > 1 && Half::of(command).is_some() {
                screens.len() * 2
            } else {
                1
            }
        }
    }
}

/// A half's slot this press: its own edge on the host, or one walked along
/// the display strip.
fn half_placement(input: &PlacementInput, half: Half, host: Screen, step: usize) -> Placement {
    if input.cycle != WindowCycle::Displays {
        return tile_placement(
            half.fractions(SIZE_CYCLE[step]),
            host,
            sanitized_gap(input.gap, host.visible_frame),
        );
    }
    let strip = ordered_screens(&input.screens);
    // Mid-chain the window sits on another display, so counting from it would
    // overshoot.
    let origin_index = input
        .origin_screen_id
        .and_then(|id| strip.iter().position(|screen| screen.id == id))
        .or_else(|| strip.iter().position(|screen| screen.id == host.id));
    let Some(origin_index) = origin_index else {
        return tile_placement(
            half.fractions(0.5),
            host,
            sanitized_gap(input.gap, host.visible_frame),
        );
    };
    if strip.len() <= 1 {
        return tile_placement(
            half.fractions(0.5),
            host,
            sanitized_gap(input.gap, host.visible_frame),
        );
    }
    // Left and Top walk backwards, so one shortcut sweeps the whole desktop
    // in one direction.
    let leads = half.edge == HalfEdge::Leading;
    let lead_offset = if leads { 0 } else { 1 };
    let step_offset = if leads { -(step as i64) } else { step as i64 };
    let slot = wrapped(
        origin_index as i64 * 2 + lead_offset + step_offset,
        strip.len() as i64 * 2,
    ) as usize;
    let edge = if slot % 2 == 0 {
        HalfEdge::Leading
    } else {
        HalfEdge::Trailing
    };
    tile_placement(
        Half {
            axis: half.axis,
            edge,
        }
        .fractions(0.5),
        strip[slot / 2],
        sanitized_gap(input.gap, strip[slot / 2].visible_frame),
    )
}

fn tile_placement(fractions: Fractions, screen: Screen, gap: f64) -> Placement {
    let frame = tile(
        screen.visible_frame,
        fractions.x0,
        fractions.x1,
        fractions.y0,
        fractions.y1,
        sanitized_gap(gap, screen.visible_frame),
    );
    Placement {
        frame,
        screen_id: screen.id,
        anchor: fractions.anchor,
        resizes: true,
    }
}

fn centered_fractions(width: f64) -> Fractions {
    Fractions {
        x0: (1.0 - width) / 2.0,
        x1: (1.0 + width) / 2.0,
        y0: 0.0,
        y1: 1.0,
        anchor: Anchor {
            horizontal: Axis::Center,
            vertical: Axis::Min,
        },
    }
}

/// The fractional bounds of a tile command at its base position, or `None`
/// when it is not one.
fn tile_fractions(command: WindowCommandId) -> Option<Fractions> {
    if let Some(half) = Half::of(command) {
        return Some(half.fractions(0.5));
    }
    let fractions = match command {
        WindowCommandId::TopLeftQuarter => Fractions {
            x0: 0.0,
            x1: 0.5,
            y0: 0.0,
            y1: 0.5,
            anchor: Anchor::top_leading(),
        },
        WindowCommandId::TopRightQuarter => Fractions {
            x0: 0.5,
            x1: 1.0,
            y0: 0.0,
            y1: 0.5,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        },
        WindowCommandId::BottomLeftQuarter => Fractions {
            x0: 0.0,
            x1: 0.5,
            y0: 0.5,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Min,
                vertical: Axis::Max,
            },
        },
        WindowCommandId::BottomRightQuarter => Fractions {
            x0: 0.5,
            x1: 1.0,
            y0: 0.5,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Max,
            },
        },
        WindowCommandId::FirstThreeFourths => Fractions {
            x0: 0.0,
            x1: 0.75,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::top_leading(),
        },
        WindowCommandId::LastThreeFourths => Fractions {
            x0: 0.25,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        },
        WindowCommandId::FirstThird => Fractions {
            x0: 0.0,
            x1: ONE_THIRD,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::top_leading(),
        },
        WindowCommandId::CenterThird => Fractions {
            x0: ONE_THIRD,
            x1: TWO_THIRDS,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Center,
                vertical: Axis::Min,
            },
        },
        WindowCommandId::LastThird => Fractions {
            x0: TWO_THIRDS,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        },
        WindowCommandId::FirstTwoThirds => Fractions {
            x0: 0.0,
            x1: TWO_THIRDS,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor::top_leading(),
        },
        WindowCommandId::LastTwoThirds => Fractions {
            x0: ONE_THIRD,
            x1: 1.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Max,
                vertical: Axis::Min,
            },
        },
        // Half the screen's area, so it reads as the family sibling of
        // Center Third.
        WindowCommandId::CenterHalf => centered_fractions(0.5),
        WindowCommandId::CenterTwoThirds => Fractions {
            x0: ONE_THIRD / 2.0,
            x1: 1.0 - ONE_THIRD / 2.0,
            y0: 0.0,
            y1: 1.0,
            anchor: Anchor {
                horizontal: Axis::Center,
                vertical: Axis::Min,
            },
        },
        _ => return None,
    };
    Some(fractions)
}

/// Whether the command lands on the fractional grid, which a display move can
/// re-derive.
pub fn is_tile_command(command: WindowCommandId) -> bool {
    tile_fractions(command).is_some()
}

/// A tile from fractional bounds of a visible frame.
///
/// The gap rule: an edge next to the screen border gets the full gap, an edge
/// between two windows gets half, so two windows side by side have exactly
/// one gap between them and every screen edge is inset by one gap.
pub fn tile(visible: Rect, x0: f64, x1: f64, y0: f64, y1: f64, gap: f64) -> Rect {
    let left = visible.min_x() + x0 * visible.width() + if x0 == 0.0 { gap } else { gap / 2.0 };
    let right = visible.min_x() + x1 * visible.width() - if x1 == 1.0 { gap } else { gap / 2.0 };
    let top = visible.min_y() + y0 * visible.height() + if y0 == 0.0 { gap } else { gap / 2.0 };
    let bottom = visible.min_y() + y1 * visible.height() - if y1 == 1.0 { gap } else { gap / 2.0 };
    rounded(Rect::new(
        left,
        top,
        (right - left).max(1.0),
        (bottom - top).max(1.0),
    ))
}

/// The box the free-floating commands work in: the full gap on every side,
/// not the grid's halves.
pub fn canvas(visible: Rect, gap: f64) -> Rect {
    rounded(Rect::new(
        visible.min_x() + gap,
        visible.min_y() + gap,
        (visible.width() - 2.0 * gap).max(0.0),
        (visible.height() - 2.0 * gap).max(0.0),
    ))
}

/// Never let repeated shrinking collapse a window to nothing.
fn minimum_size(canvas: Rect) -> (f64, f64) {
    (
        canvas.width().min((200.0f64).max(canvas.width() * 0.15)),
        canvas.height().min((150.0f64).max(canvas.height() * 0.15)),
    )
}

/// Even, so each edge moves a whole point and the two commands stay exactly
/// invertible.
fn even_step(dimension: f64) -> f64 {
    ((dimension * STEP_FRACTION / 2.0).round() * 2.0).max(2.0)
}

/// About the centre, by a fraction of the screen.
fn resized(frame: Rect, canvas: Rect, larger: bool) -> Rect {
    let direction = if larger { 1.0 } else { -1.0 };
    let floor_size = minimum_size(canvas);
    let width = canvas.width().min(
        floor_size
            .0
            .max(frame.width() + direction * even_step(canvas.width())),
    );
    let height = canvas.height().min(
        floor_size
            .1
            .max(frame.height() + direction * even_step(canvas.height())),
    );
    let centred = Rect::new(
        frame.min_x() - (width - frame.width()) / 2.0,
        frame.min_y() - (height - frame.height()) / 2.0,
        width,
        height,
    );
    rounded(clamped(centred, canvas))
}

fn shifted(frame: Rect, canvas: Rect, command: WindowCommandId) -> Rect {
    let dx = (canvas.width() * STEP_FRACTION).round();
    let dy = (canvas.height() * STEP_FRACTION).round();
    let mut moved = frame;
    match command {
        WindowCommandId::MoveLeft => moved.x -= dx,
        WindowCommandId::MoveRight => moved.x += dx,
        WindowCommandId::MoveUp => moved.y -= dy,
        WindowCommandId::MoveDown => moved.y += dy,
        WindowCommandId::MoveToLeftEdge => moved.x = canvas.min_x(),
        WindowCommandId::MoveToRightEdge => moved.x = canvas.max_x() - frame.width(),
        WindowCommandId::MoveToTopEdge => moved.y = canvas.min_y(),
        WindowCommandId::MoveToBottomEdge => moved.y = canvas.max_y() - frame.height(),
        _ => {}
    }
    rounded(clamped(moved, canvas))
}

/// Rounds the four edges, so two tiles sharing a boundary round it
/// identically.
pub fn rounded(rect: Rect) -> Rect {
    let min_x = rect.min_x().round();
    let min_y = rect.min_y().round();
    let max_x = rect.max_x().round();
    let max_y = rect.max_y().round();
    Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

/// Keeps a frame inside a box without resizing it; an oversized window pins
/// its leading edge.
pub fn clamped(frame: Rect, box_: Rect) -> Rect {
    let x = frame
        .min_x()
        .max(box_.min_x())
        .min((box_.min_x()).max(box_.max_x() - frame.width()));
    let y = frame
        .min_y()
        .max(box_.min_y())
        .min((box_.min_y()).max(box_.max_y() - frame.height()));
    Rect::new(x, y, frame.width(), frame.height())
}

/// A gap wider than the screen would produce zero-width tiles, so it is
/// capped before any math.
pub fn sanitized_gap(gap: f64, visible: Rect) -> f64 {
    if !gap.is_finite() || gap <= 0.0 || visible.width() <= 0.0 || visible.height() <= 0.0 {
        return 0.0;
    }
    gap.min(visible.width().min(visible.height()) / 10.0)
}

/// The gap as the Settings pane stores it: inside the offered range, in steps
/// of two.
pub fn sanitize_gap_setting(value: f64) -> f64 {
    if !value.is_finite() {
        return GAP_MIN;
    }
    let clamped = value.clamp(GAP_MIN, GAP_MAX);
    // Steps of two: round down to the nearest even number of points.
    (clamped / 2.0).floor() * 2.0
}

/// Wraps into `0..<length`, so neither a negative nor an overrun step escapes
/// the cycle.
fn wrapped(value: i64, length: i64) -> i64 {
    ((value % length) + length) % length
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(id: i64, x: f64, y: f64, width: f64, height: f64) -> Screen {
        Screen {
            id,
            frame: Rect::new(x, y, width, height),
            visible_frame: Rect::new(x, y, width, height),
        }
    }

    /// A 1000 x 800 display at the origin, and a window somewhere on it.
    fn one_screen() -> Vec<Screen> {
        vec![screen(1, 0.0, 0.0, 1000.0, 800.0)]
    }

    fn input(command: WindowCommandId, window: Rect) -> PlacementInput {
        PlacementInput::new(command, window, one_screen(), 0.0)
    }

    #[test]
    fn the_left_half_takes_the_left_half_of_the_display() {
        let placement = placement(&input(
            WindowCommandId::LeftHalf,
            Rect::new(100.0, 100.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame, Rect::new(0.0, 0.0, 500.0, 800.0));
        assert_eq!(placement.screen_id, 1);
        assert!(placement.resizes);
    }

    #[test]
    fn the_right_half_takes_the_right_half() {
        let placement = placement(&input(
            WindowCommandId::RightHalf,
            Rect::new(0.0, 0.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame, Rect::new(500.0, 0.0, 500.0, 800.0));
    }

    #[test]
    fn the_top_and_bottom_halves_split_the_height() {
        let top = placement(&input(
            WindowCommandId::TopHalf,
            Rect::new(0.0, 0.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(top.frame, Rect::new(0.0, 0.0, 1000.0, 400.0));
        let bottom = placement(&input(
            WindowCommandId::BottomHalf,
            Rect::new(0.0, 0.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(bottom.frame, Rect::new(0.0, 400.0, 1000.0, 400.0));
    }

    #[test]
    fn the_four_quarters_split_both_axes() {
        let cases = [
            (
                WindowCommandId::TopLeftQuarter,
                Rect::new(0.0, 0.0, 500.0, 400.0),
            ),
            (
                WindowCommandId::TopRightQuarter,
                Rect::new(500.0, 0.0, 500.0, 400.0),
            ),
            (
                WindowCommandId::BottomLeftQuarter,
                Rect::new(0.0, 400.0, 500.0, 400.0),
            ),
            (
                WindowCommandId::BottomRightQuarter,
                Rect::new(500.0, 400.0, 500.0, 400.0),
            ),
        ];
        for (command, expected) in cases {
            let placement =
                placement(&input(command, Rect::new(0.0, 0.0, 100.0, 100.0))).expect("a placement");
            assert_eq!(placement.frame, expected, "{command:?}");
        }
    }

    #[test]
    fn the_thirds_tile_the_width_without_gaps() {
        let cases = [
            (
                WindowCommandId::FirstThird,
                Rect::new(0.0, 0.0, 333.0, 800.0),
            ),
            (
                WindowCommandId::CenterThird,
                Rect::new(333.0, 0.0, 334.0, 800.0),
            ),
            (
                WindowCommandId::LastThird,
                Rect::new(667.0, 0.0, 333.0, 800.0),
            ),
        ];
        for (command, expected) in cases {
            let placement =
                placement(&input(command, Rect::new(0.0, 0.0, 100.0, 100.0))).expect("a placement");
            assert_eq!(placement.frame, expected, "{command:?}");
        }
    }

    #[test]
    fn the_two_thirds_take_the_rest_of_the_width() {
        let first = placement(&input(
            WindowCommandId::FirstTwoThirds,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(first.frame.width(), 667.0);
        let last = placement(&input(
            WindowCommandId::LastTwoThirds,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(last.frame.width(), 667.0);
        assert_eq!(last.frame.min_x(), 333.0);
    }

    #[test]
    fn the_three_fourths_overlap_in_the_middle_by_a_quarter() {
        let first = placement(&input(
            WindowCommandId::FirstThreeFourths,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(first.frame.width(), 750.0);
        let last = placement(&input(
            WindowCommandId::LastThreeFourths,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(last.frame.min_x(), 250.0);
        assert_eq!(last.frame.width(), 750.0);
    }

    #[test]
    fn the_center_half_is_half_the_width_at_full_height() {
        let placement = placement(&input(
            WindowCommandId::CenterHalf,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame, Rect::new(250.0, 0.0, 500.0, 800.0));
    }

    #[test]
    fn the_center_two_thirds_is_two_thirds_at_full_height() {
        let placement = placement(&input(
            WindowCommandId::CenterTwoThirds,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        // Two thirds of the width, rounded to whole points.
        assert_eq!(placement.frame.width(), 666.0);
        assert_eq!(placement.frame.min_x(), 167.0);
    }

    #[test]
    fn maximize_fills_the_display() {
        let placement = placement(&input(
            WindowCommandId::Maximize,
            Rect::new(10.0, 10.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame, Rect::new(0.0, 0.0, 1000.0, 800.0));
    }

    #[test]
    fn almost_maximize_leaves_a_tenth_around_the_window() {
        let placement = placement(&input(
            WindowCommandId::AlmostMaximize,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.width(), 900.0);
        assert_eq!(placement.frame.height(), 720.0);
        assert_eq!(placement.frame.min_x(), 50.0);
    }

    #[test]
    fn reasonable_size_is_sixty_per_cent_of_the_display() {
        let placement = placement(&input(
            WindowCommandId::ReasonableSize,
            Rect::new(0.0, 0.0, 100.0, 100.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.width(), 600.0);
        assert_eq!(placement.frame.height(), 480.0);
        assert_eq!(placement.frame.min_x(), 200.0);
    }

    #[test]
    fn reasonable_size_never_grows_past_the_ceiling() {
        let big = vec![screen(1, 0.0, 0.0, 4000.0, 3000.0)];
        let request = PlacementInput::new(
            WindowCommandId::ReasonableSize,
            Rect::new(0.0, 0.0, 100.0, 100.0),
            big,
            0.0,
        );
        let placement = placement(&request).expect("a placement");
        assert_eq!(placement.frame.width(), 1025.0);
        assert_eq!(placement.frame.height(), 900.0);
    }

    #[test]
    fn center_keeps_the_window_size_and_centres_it() {
        let placement = placement(&input(
            WindowCommandId::Center,
            Rect::new(0.0, 0.0, 400.0, 200.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame, Rect::new(300.0, 300.0, 400.0, 200.0));
    }

    #[test]
    fn maximize_height_keeps_the_width_and_clamps_it_on_screen() {
        let placement = placement(&input(
            WindowCommandId::MaximizeHeight,
            Rect::new(900.0, 10.0, 400.0, 200.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.height(), 800.0);
        assert_eq!(placement.frame.width(), 400.0);
        assert_eq!(placement.frame.max_x(), 1000.0);
    }

    #[test]
    fn maximize_width_keeps_the_height_and_clamps_it_on_screen() {
        let placement = placement(&input(
            WindowCommandId::MaximizeWidth,
            Rect::new(900.0, 10.0, 400.0, 200.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.width(), 1000.0);
        assert_eq!(placement.frame.height(), 200.0);
        assert_eq!(
            placement.frame.max_x(),
            1000.0,
            "it was pulled back on screen"
        );
    }

    #[test]
    fn make_larger_and_make_smaller_are_exactly_invertible() {
        let start = Rect::new(100.0, 100.0, 400.0, 300.0);
        let larger = placement(&input(WindowCommandId::MakeLarger, start)).expect("a placement");
        let smaller =
            placement(&input(WindowCommandId::MakeSmaller, larger.frame)).expect("a placement");
        assert_eq!(smaller.frame, start);
    }

    #[test]
    fn make_smaller_stops_at_the_floor() {
        let mut frame = Rect::new(0.0, 0.0, 200.0, 150.0);
        for _ in 0..50 {
            let placement =
                placement(&input(WindowCommandId::MakeSmaller, frame)).expect("a placement");
            frame = placement.frame;
        }
        assert!(frame.width() >= 200.0);
        assert!(frame.height() >= 150.0);
    }

    #[test]
    fn a_nudge_moves_the_window_without_resizing_it() {
        let placement = placement(&input(
            WindowCommandId::MoveLeft,
            Rect::new(500.0, 400.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert!(!placement.resizes);
        assert_eq!(placement.frame.width(), 400.0);
        // Five per cent of the screen, so 50 points on a 1000 wide display.
        assert_eq!(placement.frame.min_x(), 450.0);
    }

    #[test]
    fn a_nudge_stops_at_the_edge_of_the_display() {
        let placement = placement(&input(
            WindowCommandId::MoveLeft,
            Rect::new(10.0, 400.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.min_x(), 0.0);
    }

    #[test]
    fn an_edge_move_puts_the_window_against_the_edge() {
        let placement = placement(&input(
            WindowCommandId::MoveToRightEdge,
            Rect::new(100.0, 100.0, 400.0, 300.0),
        ))
        .expect("a placement");
        assert_eq!(placement.frame.max_x(), 1000.0);
        assert!(!placement.resizes);
    }

    #[test]
    fn a_gap_insets_the_screen_edge_by_one_gap_and_splits_the_middle_by_half() {
        let visible = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let left = tile(visible, 0.0, 0.5, 0.0, 1.0, 20.0);
        let right = tile(visible, 0.5, 1.0, 0.0, 1.0, 20.0);
        assert_eq!(left.min_x(), 20.0);
        assert_eq!(right.max_x(), 980.0);
        // The edge between the two tiles gives half a gap each, so exactly one
        // gap separates them.
        assert_eq!(right.min_x() - left.max_x(), 20.0);
    }

    #[test]
    fn a_gap_is_capped_at_a_tenth_of_the_display() {
        let visible = Rect::new(0.0, 0.0, 1000.0, 800.0);
        assert_eq!(sanitized_gap(500.0, visible), 80.0);
        assert_eq!(sanitized_gap(-4.0, visible), 0.0);
        assert_eq!(sanitized_gap(f64::NAN, visible), 0.0);
    }

    #[test]
    fn the_gap_setting_is_held_to_its_range_and_step() {
        assert_eq!(sanitize_gap_setting(-10.0), 0.0);
        assert_eq!(sanitize_gap_setting(100.0), 64.0);
        assert_eq!(sanitize_gap_setting(11.0), 10.0);
        assert_eq!(sanitize_gap_setting(12.0), 12.0);
    }

    #[test]
    fn two_tiles_never_overlap_and_never_leave_a_line_between_them() {
        let visible = Rect::new(0.0, 0.0, 1001.0, 800.0);
        let left = tile(visible, 0.0, 0.5, 0.0, 1.0, 0.0);
        let right = tile(visible, 0.5, 1.0, 0.0, 1.0, 0.0);
        assert_eq!(left.max_x(), right.min_x());
    }

    #[test]
    fn the_cycling_half_steps_through_half_a_third_and_two_thirds() {
        let mut request = input(WindowCommandId::LeftHalf, Rect::new(0.0, 0.0, 100.0, 100.0));
        request.cycle = WindowCycle::Sizes;
        let widths: Vec<f64> = (0..3)
            .map(|step| {
                let mut step_request = request.clone();
                step_request.step = step;
                placement(&step_request).expect("a placement").frame.width()
            })
            .collect();
        assert_eq!(widths, vec![500.0, 333.0, 667.0]);
    }

    #[test]
    fn without_cycling_a_half_keeps_its_frame() {
        let mut request = input(WindowCommandId::LeftHalf, Rect::new(0.0, 0.0, 100.0, 100.0));
        request.cycle = WindowCycle::Off;
        request.step = 2;
        let placement = placement(&request).expect("a placement");
        assert_eq!(placement.frame.width(), 500.0);
    }

    #[test]
    fn the_cycle_length_is_one_for_a_command_that_does_not_cycle() {
        assert_eq!(
            cycle_length(WindowCommandId::Maximize, &one_screen(), WindowCycle::Sizes),
            1
        );
        assert_eq!(
            cycle_length(WindowCommandId::LeftHalf, &one_screen(), WindowCycle::Sizes),
            3
        );
        assert_eq!(
            cycle_length(
                WindowCommandId::LeftHalf,
                &one_screen(),
                WindowCycle::Displays
            ),
            1,
            "one display has no strip to walk"
        );
    }

    #[test]
    fn the_display_cycle_walks_the_strip_of_displays() {
        let screens = vec![
            screen(1, 0.0, 0.0, 1000.0, 800.0),
            screen(2, 1000.0, 0.0, 1000.0, 800.0),
        ];
        // Upstream's documented sequence with two displays: Display 1 left,
        // Display 2 right, Display 2 left, Display 1 right.
        let mut request = PlacementInput::new(
            WindowCommandId::LeftHalf,
            Rect::new(0.0, 0.0, 400.0, 300.0),
            screens,
            0.0,
        );
        request.cycle = WindowCycle::Displays;

        request.step = 0;
        let first = placement(&request).expect("a placement");
        assert_eq!(first.screen_id, 1, "the first press is Display 1 left");
        assert_eq!(first.frame.min_x(), 0.0);

        request.step = 1;
        let second = placement(&request).expect("a placement");
        assert_eq!(second.screen_id, 2, "the second press is Display 2 right");
        assert_eq!(second.frame.min_x(), 1500.0);

        request.step = 2;
        let third = placement(&request).expect("a placement");
        assert_eq!(third.screen_id, 2, "the third press is Display 2 left");
        assert_eq!(third.frame.min_x(), 1000.0);

        request.step = 3;
        let fourth = placement(&request).expect("a placement");
        assert_eq!(fourth.screen_id, 1, "the fourth press is Display 1 right");
        assert_eq!(fourth.frame.min_x(), 500.0);
    }

    #[test]
    fn moving_to_the_next_display_walks_the_ordered_strip() {
        let screens = vec![
            screen(2, 1000.0, 0.0, 1000.0, 800.0),
            screen(1, 0.0, 0.0, 1000.0, 800.0),
        ];
        let request = PlacementInput::new(
            WindowCommandId::NextDisplay,
            Rect::new(100.0, 100.0, 400.0, 300.0),
            screens,
            0.0,
        );
        let placement = placement(&request).expect("a placement");
        assert_eq!(placement.screen_id, 2);
    }

    #[test]
    fn one_display_makes_a_display_move_a_no_op() {
        let request = PlacementInput::new(
            WindowCommandId::NextDisplay,
            Rect::new(100.0, 100.0, 400.0, 300.0),
            one_screen(),
            0.0,
        );
        assert!(placement(&request).is_none());
    }

    #[test]
    fn restore_puts_the_window_back_where_it_started() {
        let mut request = input(
            WindowCommandId::Restore,
            Rect::new(500.0, 400.0, 400.0, 300.0),
        );
        request.restore_frame = Some(Rect::new(10.0, 20.0, 400.0, 300.0));
        let placement = placement(&request).expect("a placement");
        assert_eq!(placement.frame, Rect::new(10.0, 20.0, 400.0, 300.0));
    }

    #[test]
    fn a_restore_point_stranded_off_every_display_comes_back_centred() {
        let mut request = input(
            WindowCommandId::Restore,
            Rect::new(5000.0, 5000.0, 400.0, 300.0),
        );
        request.restore_frame = Some(Rect::new(5000.0, 5000.0, 400.0, 300.0));
        let placement = placement(&request).expect("a placement");
        assert_eq!(placement.frame, Rect::new(300.0, 250.0, 400.0, 300.0));
    }

    #[test]
    fn a_window_without_a_position_is_not_placed() {
        let request = input(WindowCommandId::MoveLeft, Rect::new(0.0, 0.0, 0.0, 0.0));
        assert!(placement(&request).is_none());
    }

    #[test]
    fn a_command_with_no_geometry_answers_nothing() {
        let request = input(
            WindowCommandId::NextSpace,
            Rect::new(0.0, 0.0, 400.0, 300.0),
        );
        assert!(placement(&request).is_none());
    }

    #[test]
    fn a_machine_with_no_display_answers_nothing() {
        let request = PlacementInput::new(
            WindowCommandId::LeftHalf,
            Rect::new(0.0, 0.0, 400.0, 300.0),
            Vec::new(),
            0.0,
        );
        assert!(placement(&request).is_none());
    }

    #[test]
    fn the_tile_commands_are_the_ones_a_display_move_can_re_derive() {
        assert!(is_tile_command(WindowCommandId::LeftHalf));
        assert!(is_tile_command(WindowCommandId::TopRightQuarter));
        assert!(is_tile_command(WindowCommandId::CenterThird));
        assert!(!is_tile_command(WindowCommandId::Maximize));
        assert!(!is_tile_command(WindowCommandId::MoveLeft));
    }

    #[test]
    fn the_display_the_window_sits_on_is_the_one_it_overlaps_most() {
        let screens = vec![
            screen(1, 0.0, 0.0, 1000.0, 800.0),
            screen(2, 1000.0, 0.0, 1000.0, 800.0),
        ];
        // Wholly on the first display.
        let host =
            screen_containing(Rect::new(600.0, 100.0, 400.0, 300.0), &screens).expect("a display");
        assert_eq!(host.id, 1);
        // Wholly on the second.
        let host =
            screen_containing(Rect::new(1100.0, 100.0, 400.0, 300.0), &screens).expect("a display");
        assert_eq!(host.id, 2);
        // Straddling the seam: the larger overlap wins.
        let host =
            screen_containing(Rect::new(900.0, 100.0, 400.0, 300.0), &screens).expect("a display");
        assert_eq!(host.id, 2, "300 points on the second, 100 on the first");
    }
}
