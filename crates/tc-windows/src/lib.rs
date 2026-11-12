//! `tc-windows` - YAL port of Tinycast's window management.
//!
//! The window commands: the catalogue of the thirty-nine commands upstream
//! ships, and the geometry that decides where a window goes when one of them
//! runs. The geometry is pure - it takes a rectangle, a set of displays and a
//! gap, and answers a rectangle - so it can be tested without a desktop, and
//! so the Windows-specific part of the work stays a thin layer that moves a
//! real window.
//
//! Nothing here touches Win32. The mover lives in `tc-platform-win`.
//!
//! See `docs/ARCHITECTURE.md` for the crate map.

pub mod action_memory;
pub mod command;
pub mod geometry;

pub use action_memory::{Decision, Record, WindowActionMemory, DEFAULT_CAPACITY, TOLERANCE};
pub use command::{
    WindowCommand, WindowCommandCatalog, WindowCommandGroup, WindowCommandId, WindowCommandKind,
};
pub use geometry::{
    cycle_length, is_tile_command, placement, screen_containing, Anchor, Axis, Placement,
    PlacementInput, Rect, Screen, WindowCycle,
};
