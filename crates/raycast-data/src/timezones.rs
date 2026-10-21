//! The time zone list.
//!
//! The set comes from the IANA database via `chrono-tz`, so it is whatever
//! the build's `chrono-tz` version carries.

use chrono_tz::Tz;

/// One time zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeZoneEntry {
    /// The IANA identifier, e.g. `Europe/London`.
    pub name: String,
    /// The current UTC offset, formatted as `+HH:MM`.
    pub offset: String,
}

/// Every known time zone, with its current offset.
pub fn all_time_zones() -> Vec<TimeZoneEntry> {
    let now = chrono::Utc::now();
    let mut entries: Vec<TimeZoneEntry> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(|zone| {
            let offset = now.with_timezone(zone).format("%:z").to_string();
            TimeZoneEntry {
                name: zone.name().to_string(),
                offset,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Resolve a time zone by IANA identifier.
pub fn resolve(name: &str) -> Option<Tz> {
    name.parse::<Tz>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_time_zones_with_offsets() {
        let zones = all_time_zones();
        assert!(zones.len() > 100);
        assert!(zones.iter().any(|z| z.name == "Europe/London"));
        assert!(zones[0].offset.starts_with('+') || zones[0].offset.starts_with('-'));
    }

    #[test]
    fn resolves_by_name() {
        assert!(resolve("Europe/London").is_some());
        assert!(resolve("Not/AZone").is_none());
    }
}
