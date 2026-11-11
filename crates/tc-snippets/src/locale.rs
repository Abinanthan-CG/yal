//! The small locale table the date tokens format with.
//!
//! Windows has no ICU, so instead of a full localization framework each
//! locale the port supports carries its own month names, weekday names and
//! the pattern order its date and time styles use. The values here mirror
//! the CLDR data upstream's `DateFormatter` reads, restricted to the locales
//! upstream's own tests pin.

/// The month, weekday and pattern data of one locale.
pub struct LocaleData {
    /// The locale's canonical identifier.
    pub identifier: &'static str,
    /// The full month names, January first.
    pub months_full: [&'static str; 12],
    /// The abbreviated month names, January first.
    pub months_abbreviated: [&'static str; 12],
    /// The full weekday names, Sunday first.
    pub weekdays_full: [&'static str; 7],
    /// The abbreviated weekday names, Sunday first.
    pub weekdays_abbreviated: [&'static str; 7],
    /// The morning marker.
    pub am: &'static str,
    /// The afternoon marker.
    pub pm: &'static str,
    /// The pattern the medium date style uses.
    pub medium_date: &'static str,
    /// The pattern the short time style uses.
    pub short_time: &'static str,
    /// The pattern the medium date and short time combination uses.
    pub medium_datetime: &'static str,
}

/// The English locale, which every unknown locale falls back to.
const ENGLISH: LocaleData = LocaleData {
    identifier: "en_US_POSIX",
    months_full: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    months_abbreviated: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    weekdays_full: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    weekdays_abbreviated: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    am: "AM",
    pm: "PM",
    // The narrow no-break space before the marker is what CLDR 42 uses.
    medium_date: "MMM d, yyyy",
    short_time: "h:mm\u{202F}a",
    medium_datetime: "MMM d, yyyy 'at' h:mm\u{202F}a",
};

/// The French locale.
const FRENCH: LocaleData = LocaleData {
    identifier: "fr_FR",
    months_full: [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ],
    months_abbreviated: [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ],
    weekdays_full: [
        "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
    ],
    weekdays_abbreviated: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
    am: "AM",
    pm: "PM",
    medium_date: "d MMM yyyy",
    short_time: "HH:mm",
    medium_datetime: "d MMM yyyy 'à' HH:mm",
};

/// The locale data for a locale identifier.
///
/// Mirrors upstream's `Locale(identifier:)`: `fr-FR` and `fr_FR` both name
/// the French locale, and any locale the table does not know falls back to
/// English.
pub fn for_identifier(identifier: &str) -> &'static LocaleData {
    let normalized = identifier.replace('_', "-").to_lowercase();
    if normalized.starts_with("fr") {
        &FRENCH
    } else {
        &ENGLISH
    }
}
