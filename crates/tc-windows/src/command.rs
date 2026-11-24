//! The catalogue of window commands.
//!
//! Upstream ships thirty-nine window commands in eight groups, and each one
//! is a row in the launcher, a command with its own shortcut, and an entry in
//! the Settings pane. The catalogue is the single place that knows them: the
//! ids are stable strings, because a stored shortcut, a deeplink and a backup
//! entry all name a command by its id.
//!
//! The names are the English names upstream ships, and they are also the
//! localization keys: the Windows port shows them as they are until a
//! translation table says otherwise.

/// The identifier of a window command.
///
/// The string form is the stored id: it appears in `window-command:<id>` entry
/// ids, in the `hotkey.windowCommand.<id>` settings keys, and in a backup.
/// Changing one of these strings changes the user's data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowCommandId {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    FirstThreeFourths,
    LastThreeFourths,
    FirstThird,
    CenterThird,
    LastThird,
    FirstTwoThirds,
    LastTwoThirds,
    Maximize,
    AlmostMaximize,
    ReasonableSize,
    MaximizeHeight,
    MaximizeWidth,
    Center,
    CenterHalf,
    CenterTwoThirds,
    MakeLarger,
    MakeSmaller,
    Restore,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveToLeftEdge,
    MoveToRightEdge,
    MoveToTopEdge,
    MoveToBottomEdge,
    NextDisplay,
    PreviousDisplay,
    ToggleFullscreen,
    PreviousSpace,
    NextSpace,
    AlwaysOnTop,
}

/// The group a window command is listed under.
///
/// The groups are also the order the Settings pane lists them in, so the
/// order of the variants is the order on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowCommandGroup {
    Halves,
    Quarters,
    Fourths,
    Thirds,
    Sizing,
    Moving,
    Fullscreen,
    Spaces,
}

impl WindowCommandGroup {
    /// The name the group is shown under.
    pub fn label(self) -> &'static str {
        match self {
            WindowCommandGroup::Halves => "Halves",
            WindowCommandGroup::Quarters => "Quarters",
            WindowCommandGroup::Fourths => "Fourths",
            WindowCommandGroup::Thirds => "Thirds",
            WindowCommandGroup::Sizing => "Sizing",
            WindowCommandGroup::Moving => "Moving",
            WindowCommandGroup::Fullscreen => "Fullscreen",
            WindowCommandGroup::Spaces => "Spaces",
        }
    }
}

/// What a command has to do, so the mover's dispatch stays exhaustive over
/// the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommandKind {
    /// The command computes a rectangle and the window is moved and resized
    /// into it.
    Geometry,
    /// The command puts the window back where it started, so the rectangle
    /// comes from a remembered frame rather than from the display.
    Restore,
    /// The command asks the window to go fullscreen; no rectangle is computed.
    Fullscreen,
    /// The command changes the virtual desktop; no window is involved.
    Space,
    /// The command toggles whether the window stays above other windows; no
    /// rectangle is computed.
    AlwaysOnTop,
}

/// One window command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCommand {
    /// The command's identifier.
    pub id: WindowCommandId,
    /// The name shown in the launcher and in Settings.
    pub name: &'static str,
    /// The group the command is listed under.
    pub group: WindowCommandGroup,
    /// What the command has to do.
    pub kind: WindowCommandKind,
    /// Whether pressing the command again walks it through its cycle.
    ///
    /// Only the four halves and Center Half cycle; every other command either
    /// does the same thing again or was never a cycling command.
    pub cycles_on_repeat: bool,
    /// Whether the command resizes the window as well as moving it.
    ///
    /// The nudges and the edge moves only move, so the mover never writes a
    /// size for them.
    pub resizes: bool,
}

impl WindowCommand {
    /// The entry id a launcher row, a shortcut or a backup entry uses.
    pub fn entry_id(&self) -> String {
        format!(
            "{}{}",
            tc_core::entry::PREFIX_WINDOW_COMMAND,
            self.id.as_str()
        )
    }
}

impl WindowCommandId {
    /// Every command, in the order upstream declares them, which is the
    /// order the Settings pane lists them in.
    pub const ALL: [WindowCommandId; 40] = [
        WindowCommandId::LeftHalf,
        WindowCommandId::RightHalf,
        WindowCommandId::TopHalf,
        WindowCommandId::BottomHalf,
        WindowCommandId::TopLeftQuarter,
        WindowCommandId::TopRightQuarter,
        WindowCommandId::BottomLeftQuarter,
        WindowCommandId::BottomRightQuarter,
        WindowCommandId::FirstThreeFourths,
        WindowCommandId::LastThreeFourths,
        WindowCommandId::FirstThird,
        WindowCommandId::CenterThird,
        WindowCommandId::LastThird,
        WindowCommandId::FirstTwoThirds,
        WindowCommandId::LastTwoThirds,
        WindowCommandId::Maximize,
        WindowCommandId::AlmostMaximize,
        WindowCommandId::ReasonableSize,
        WindowCommandId::MaximizeHeight,
        WindowCommandId::MaximizeWidth,
        WindowCommandId::Center,
        WindowCommandId::CenterHalf,
        WindowCommandId::CenterTwoThirds,
        WindowCommandId::MakeLarger,
        WindowCommandId::MakeSmaller,
        WindowCommandId::Restore,
        WindowCommandId::MoveLeft,
        WindowCommandId::MoveRight,
        WindowCommandId::MoveUp,
        WindowCommandId::MoveDown,
        WindowCommandId::MoveToLeftEdge,
        WindowCommandId::MoveToRightEdge,
        WindowCommandId::MoveToTopEdge,
        WindowCommandId::MoveToBottomEdge,
        WindowCommandId::NextDisplay,
        WindowCommandId::PreviousDisplay,
        WindowCommandId::ToggleFullscreen,
        WindowCommandId::PreviousSpace,
        WindowCommandId::NextSpace,
        WindowCommandId::AlwaysOnTop,
    ];

    /// The stored id.
    pub fn as_str(self) -> &'static str {
        match self {
            WindowCommandId::LeftHalf => "left-half",
            WindowCommandId::RightHalf => "right-half",
            WindowCommandId::TopHalf => "top-half",
            WindowCommandId::BottomHalf => "bottom-half",
            WindowCommandId::TopLeftQuarter => "top-left-quarter",
            WindowCommandId::TopRightQuarter => "top-right-quarter",
            WindowCommandId::BottomLeftQuarter => "bottom-left-quarter",
            WindowCommandId::BottomRightQuarter => "bottom-right-quarter",
            WindowCommandId::FirstThreeFourths => "first-three-fourths",
            WindowCommandId::LastThreeFourths => "last-three-fourths",
            WindowCommandId::FirstThird => "first-third",
            WindowCommandId::CenterThird => "center-third",
            WindowCommandId::LastThird => "last-third",
            WindowCommandId::FirstTwoThirds => "first-two-thirds",
            WindowCommandId::LastTwoThirds => "last-two-thirds",
            WindowCommandId::Maximize => "maximize",
            WindowCommandId::AlmostMaximize => "almost-maximize",
            WindowCommandId::ReasonableSize => "reasonable-size",
            WindowCommandId::MaximizeHeight => "maximize-height",
            WindowCommandId::MaximizeWidth => "maximize-width",
            WindowCommandId::Center => "center",
            WindowCommandId::CenterHalf => "center-half",
            WindowCommandId::CenterTwoThirds => "center-two-thirds",
            WindowCommandId::MakeLarger => "make-larger",
            WindowCommandId::MakeSmaller => "make-smaller",
            WindowCommandId::Restore => "restore",
            WindowCommandId::MoveLeft => "move-left",
            WindowCommandId::MoveRight => "move-right",
            WindowCommandId::MoveUp => "move-up",
            WindowCommandId::MoveDown => "move-down",
            WindowCommandId::MoveToLeftEdge => "move-to-left-edge",
            WindowCommandId::MoveToRightEdge => "move-to-right-edge",
            WindowCommandId::MoveToTopEdge => "move-to-top-edge",
            WindowCommandId::MoveToBottomEdge => "move-to-bottom-edge",
            WindowCommandId::NextDisplay => "next-display",
            WindowCommandId::PreviousDisplay => "previous-display",
            WindowCommandId::ToggleFullscreen => "toggle-fullscreen",
            WindowCommandId::PreviousSpace => "previous-space",
            WindowCommandId::NextSpace => "next-space",
            WindowCommandId::AlwaysOnTop => "always-on-top",
        }
    }

    /// Look a command up by its stored id.
    pub fn parse(value: &str) -> Option<WindowCommandId> {
        WindowCommandId::ALL
            .into_iter()
            .find(|command| command.as_str() == value)
    }

    /// The English name, which is also the localization key.
    pub fn english_name(self) -> &'static str {
        match self {
            WindowCommandId::LeftHalf => "Left Half",
            WindowCommandId::RightHalf => "Right Half",
            WindowCommandId::TopHalf => "Top Half",
            WindowCommandId::BottomHalf => "Bottom Half",
            WindowCommandId::TopLeftQuarter => "Top Left Quarter",
            WindowCommandId::TopRightQuarter => "Top Right Quarter",
            WindowCommandId::BottomLeftQuarter => "Bottom Left Quarter",
            WindowCommandId::BottomRightQuarter => "Bottom Right Quarter",
            WindowCommandId::FirstThreeFourths => "First Three Fourths",
            WindowCommandId::LastThreeFourths => "Last Three Fourths",
            WindowCommandId::FirstThird => "First Third",
            WindowCommandId::CenterThird => "Center Third",
            WindowCommandId::LastThird => "Last Third",
            WindowCommandId::FirstTwoThirds => "First Two Thirds",
            WindowCommandId::LastTwoThirds => "Last Two Thirds",
            WindowCommandId::Maximize => "Maximize",
            WindowCommandId::AlmostMaximize => "Almost Maximize",
            WindowCommandId::ReasonableSize => "Reasonable Size",
            WindowCommandId::MaximizeHeight => "Maximize Height",
            WindowCommandId::MaximizeWidth => "Maximize Width",
            WindowCommandId::Center => "Center",
            WindowCommandId::CenterHalf => "Center Half",
            WindowCommandId::CenterTwoThirds => "Center Two Thirds",
            WindowCommandId::MakeLarger => "Make Larger",
            WindowCommandId::MakeSmaller => "Make Smaller",
            WindowCommandId::Restore => "Restore Window",
            WindowCommandId::MoveLeft => "Move Left",
            WindowCommandId::MoveRight => "Move Right",
            WindowCommandId::MoveUp => "Move Up",
            WindowCommandId::MoveDown => "Move Down",
            WindowCommandId::MoveToLeftEdge => "Move to Left Edge",
            WindowCommandId::MoveToRightEdge => "Move to Right Edge",
            WindowCommandId::MoveToTopEdge => "Move to Top Edge",
            WindowCommandId::MoveToBottomEdge => "Move to Bottom Edge",
            WindowCommandId::NextDisplay => "Move to Next Display",
            WindowCommandId::PreviousDisplay => "Move to Previous Display",
            WindowCommandId::ToggleFullscreen => "Toggle Fullscreen",
            WindowCommandId::PreviousSpace => "Switch to Previous Space",
            WindowCommandId::NextSpace => "Switch to Next Space",
            WindowCommandId::AlwaysOnTop => "Toggle Always on Top",
        }
    }

    /// The group the command is listed under.
    pub fn group(self) -> WindowCommandGroup {
        match self {
            WindowCommandId::LeftHalf
            | WindowCommandId::RightHalf
            | WindowCommandId::TopHalf
            | WindowCommandId::BottomHalf => WindowCommandGroup::Halves,
            WindowCommandId::TopLeftQuarter
            | WindowCommandId::TopRightQuarter
            | WindowCommandId::BottomLeftQuarter
            | WindowCommandId::BottomRightQuarter => WindowCommandGroup::Quarters,
            WindowCommandId::FirstThreeFourths | WindowCommandId::LastThreeFourths => {
                WindowCommandGroup::Fourths
            }
            WindowCommandId::FirstThird
            | WindowCommandId::CenterThird
            | WindowCommandId::LastThird
            | WindowCommandId::FirstTwoThirds
            | WindowCommandId::LastTwoThirds => WindowCommandGroup::Thirds,
            WindowCommandId::Maximize
            | WindowCommandId::AlmostMaximize
            | WindowCommandId::ReasonableSize
            | WindowCommandId::MaximizeHeight
            | WindowCommandId::MaximizeWidth
            | WindowCommandId::Center
            | WindowCommandId::CenterHalf
            | WindowCommandId::CenterTwoThirds
            | WindowCommandId::MakeLarger
            | WindowCommandId::MakeSmaller
            | WindowCommandId::Restore
            | WindowCommandId::AlwaysOnTop => WindowCommandGroup::Sizing,
            WindowCommandId::MoveLeft
            | WindowCommandId::MoveRight
            | WindowCommandId::MoveUp
            | WindowCommandId::MoveDown
            | WindowCommandId::MoveToLeftEdge
            | WindowCommandId::MoveToRightEdge
            | WindowCommandId::MoveToTopEdge
            | WindowCommandId::MoveToBottomEdge
            | WindowCommandId::NextDisplay
            | WindowCommandId::PreviousDisplay => WindowCommandGroup::Moving,
            WindowCommandId::ToggleFullscreen => WindowCommandGroup::Fullscreen,
            WindowCommandId::PreviousSpace | WindowCommandId::NextSpace => {
                WindowCommandGroup::Spaces
            }
        }
    }

    /// What the command has to do.
    pub fn kind(self) -> WindowCommandKind {
        match self {
            WindowCommandId::Restore => WindowCommandKind::Restore,
            WindowCommandId::ToggleFullscreen => WindowCommandKind::Fullscreen,
            WindowCommandId::PreviousSpace | WindowCommandId::NextSpace => WindowCommandKind::Space,
            WindowCommandId::AlwaysOnTop => WindowCommandKind::AlwaysOnTop,
            _ => WindowCommandKind::Geometry,
        }
    }

    /// Whether pressing the command again walks it through its cycle.
    ///
    /// Upstream cycles the four halves and Center Half, and nothing else.
    pub fn cycles_on_repeat(self) -> bool {
        matches!(
            self,
            WindowCommandId::LeftHalf
                | WindowCommandId::RightHalf
                | WindowCommandId::TopHalf
                | WindowCommandId::BottomHalf
                | WindowCommandId::CenterHalf
        )
    }

    /// Whether the command resizes the window as well as moving it.
    ///
    /// The nudges and the edge moves only move.
    pub fn resizes(self) -> bool {
        !matches!(
            self,
            WindowCommandId::MoveLeft
                | WindowCommandId::MoveRight
                | WindowCommandId::MoveUp
                | WindowCommandId::MoveDown
                | WindowCommandId::MoveToLeftEdge
                | WindowCommandId::MoveToRightEdge
                | WindowCommandId::MoveToTopEdge
                | WindowCommandId::MoveToBottomEdge
        )
    }

    /// The entry id a launcher row, a shortcut or a backup entry uses.
    pub fn entry_id(self) -> String {
        format!("{}{}", tc_core::entry::PREFIX_WINDOW_COMMAND, self.as_str())
    }
}

/// The catalogue: every window command, in the order upstream declares them.
pub struct WindowCommandCatalog;

impl WindowCommandCatalog {
    /// Every command, in declaration order.
    pub fn all() -> Vec<WindowCommand> {
        WindowCommandId::ALL
            .into_iter()
            .map(WindowCommand::of)
            .collect()
    }

    /// One command, looked up by its id.
    pub fn command(id: WindowCommandId) -> WindowCommand {
        WindowCommand::of(id)
    }

    /// One command, looked up by the id inside an entry id.
    pub fn command_for_entry_id(entry_id: &str) -> Option<WindowCommand> {
        let rest = entry_id.strip_prefix(tc_core::entry::PREFIX_WINDOW_COMMAND)?;
        WindowCommandId::parse(rest).map(WindowCommand::of)
    }

    /// The commands of one group, in declaration order.
    pub fn group(group: WindowCommandGroup) -> Vec<WindowCommand> {
        WindowCommandCatalog::all()
            .into_iter()
            .filter(|command| command.group == group)
            .collect()
    }

    /// The groups, in the order the Settings pane lists them, each with its
    /// commands. Groups with no commands are left out.
    pub fn grouped() -> Vec<(WindowCommandGroup, Vec<WindowCommand>)> {
        [
            WindowCommandGroup::Halves,
            WindowCommandGroup::Quarters,
            WindowCommandGroup::Fourths,
            WindowCommandGroup::Thirds,
            WindowCommandGroup::Sizing,
            WindowCommandGroup::Moving,
            WindowCommandGroup::Fullscreen,
            WindowCommandGroup::Spaces,
        ]
        .into_iter()
        .filter_map(|group| {
            let commands = WindowCommandCatalog::group(group);
            if commands.is_empty() {
                None
            } else {
                Some((group, commands))
            }
        })
        .collect()
    }
}

impl WindowCommand {
    /// One command, built from its id.
    pub fn of(id: WindowCommandId) -> WindowCommand {
        WindowCommand {
            id,
            name: id.english_name(),
            group: id.group(),
            kind: id.kind(),
            cycles_on_repeat: id.cycles_on_repeat(),
            resizes: id.resizes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn there_are_exactly_forty_commands() {
        assert_eq!(WindowCommandId::ALL.len(), 40);
        assert_eq!(WindowCommandCatalog::all().len(), 40);
    }

    #[test]
    fn the_groups_count_up_to_forty() {
        let counts: Vec<(WindowCommandGroup, usize)> = [
            WindowCommandGroup::Halves,
            WindowCommandGroup::Quarters,
            WindowCommandGroup::Fourths,
            WindowCommandGroup::Thirds,
            WindowCommandGroup::Sizing,
            WindowCommandGroup::Moving,
            WindowCommandGroup::Fullscreen,
            WindowCommandGroup::Spaces,
        ]
        .into_iter()
        .map(|group| (group, WindowCommandCatalog::group(group).len()))
        .collect();
        assert_eq!(
            counts,
            vec![
                (WindowCommandGroup::Halves, 4),
                (WindowCommandGroup::Quarters, 4),
                (WindowCommandGroup::Fourths, 2),
                (WindowCommandGroup::Thirds, 5),
                (WindowCommandGroup::Sizing, 12),
                (WindowCommandGroup::Moving, 10),
                (WindowCommandGroup::Fullscreen, 1),
                (WindowCommandGroup::Spaces, 2),
            ]
        );
        assert_eq!(counts.iter().map(|(_, count)| count).sum::<usize>(), 40);
    }

    #[test]
    fn the_stored_ids_are_unique_and_round_trip() {
        let ids: HashSet<&str> = WindowCommandId::ALL
            .into_iter()
            .map(WindowCommandId::as_str)
            .collect();
        assert_eq!(ids.len(), 40);
        for command in WindowCommandId::ALL {
            assert_eq!(WindowCommandId::parse(command.as_str()), Some(command));
        }
        assert_eq!(
            WindowCommandId::parse("always-on-top"),
            Some(WindowCommandId::AlwaysOnTop)
        );
        assert_eq!(WindowCommandId::parse("no-such-command"), None);
    }

    #[test]
    fn the_names_are_unique() {
        let names: HashSet<&str> = WindowCommandId::ALL
            .into_iter()
            .map(WindowCommandId::english_name)
            .collect();
        assert_eq!(names.len(), 40);
    }

    #[test]
    fn the_five_cycling_commands_are_the_halves_and_the_center_half() {
        let cycling: Vec<WindowCommandId> = WindowCommandId::ALL
            .into_iter()
            .filter(|command| command.cycles_on_repeat())
            .collect();
        assert_eq!(
            cycling,
            vec![
                WindowCommandId::LeftHalf,
                WindowCommandId::RightHalf,
                WindowCommandId::TopHalf,
                WindowCommandId::BottomHalf,
                WindowCommandId::CenterHalf,
            ]
        );
    }

    #[test]
    fn the_eight_moving_commands_do_not_resize() {
        let moving_only: Vec<WindowCommandId> = WindowCommandId::ALL
            .into_iter()
            .filter(|command| !command.resizes())
            .collect();
        assert_eq!(moving_only.len(), 8);
        for command in moving_only {
            assert_eq!(command.group(), WindowCommandGroup::Moving);
        }
    }

    #[test]
    fn the_three_special_commands_have_their_own_kinds() {
        assert_eq!(WindowCommandId::Restore.kind(), WindowCommandKind::Restore);
        assert_eq!(
            WindowCommandId::ToggleFullscreen.kind(),
            WindowCommandKind::Fullscreen
        );
        assert_eq!(
            WindowCommandId::PreviousSpace.kind(),
            WindowCommandKind::Space
        );
        assert_eq!(WindowCommandId::NextSpace.kind(), WindowCommandKind::Space);
        assert_eq!(
            WindowCommandId::AlwaysOnTop.kind(),
            WindowCommandKind::AlwaysOnTop
        );
        assert_eq!(
            WindowCommandId::LeftHalf.kind(),
            WindowCommandKind::Geometry
        );
    }

    #[test]
    fn an_entry_id_carries_the_command_id() {
        let command = WindowCommandCatalog::command(WindowCommandId::TopLeftQuarter);
        assert_eq!(command.entry_id(), "window-command:top-left-quarter");
        assert_eq!(
            WindowCommandCatalog::command_for_entry_id("window-command:top-left-quarter"),
            Some(command)
        );
        assert!(WindowCommandCatalog::command_for_entry_id("window-command:nope").is_none());
        assert!(WindowCommandCatalog::command_for_entry_id("command:left-half").is_none());
    }

    #[test]
    fn the_entry_id_resolves_to_the_action_that_runs_it() {
        let entry_id = WindowCommandId::LeftHalf.entry_id();
        let action = tc_core::entry::action_for_entry_id(&entry_id).expect("the action");
        assert_eq!(
            action,
            tc_core::entry::HotKeyAction::WindowCommand {
                id: "left-half".to_string()
            }
        );
    }

    #[test]
    fn the_grouped_listing_keeps_the_settings_order() {
        let groups: Vec<WindowCommandGroup> = WindowCommandCatalog::grouped()
            .into_iter()
            .map(|(group, _)| group)
            .collect();
        assert_eq!(
            groups,
            vec![
                WindowCommandGroup::Halves,
                WindowCommandGroup::Quarters,
                WindowCommandGroup::Fourths,
                WindowCommandGroup::Thirds,
                WindowCommandGroup::Sizing,
                WindowCommandGroup::Moving,
                WindowCommandGroup::Fullscreen,
                WindowCommandGroup::Spaces,
            ]
        );
    }
}
