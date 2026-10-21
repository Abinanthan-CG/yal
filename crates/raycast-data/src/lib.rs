//! Static datasets.
//!
//! Everything here is bundled with the build. Nothing in this crate performs
//! I/O or reaches the network, which keeps the palette instant and keeps the
//! launcher usable with no connectivity.

pub mod colors;
pub mod emoji;
pub mod password;
pub mod timezones;

pub use colors::{Color, ColorError, Hsl, NAMED_COLORS, by_name as color_by_name, parse as parse_color};
pub use emoji::Emoji;
pub use password::{PasswordOptions, generate as generate_password};
pub use timezones::{TimeZoneEntry, all_time_zones};
