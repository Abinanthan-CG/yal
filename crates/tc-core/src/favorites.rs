//! The launcher's favorites slots: the number keys that open a favorite.
//!
//! Upstream calls this `FavoriteSlots`. Ten digits is the ceiling: the tenth
//! favorite is opened with the digit `0`, and a favorite past it is listed but
//! carries no chord.
//!
//! The number row is addressed by its ANSI key codes rather than by the
//! character the key produces, so the slots work the same on a layout where
//! the number row is somewhere else.

/// The slots a favorite can be opened from.
///
/// `⌘1`…`⌘9` open the first nine favorites, and `⌘0` opens the tenth.
pub struct FavoriteSlots;

impl FavoriteSlots {
    /// The digit each slot is opened with, in slot order.
    ///
    /// The tenth slot is `0`, not `10`: the number row has no eleventh key.
    pub const DIGITS: [char; 10] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

    /// The ANSI number-row key codes in visual order `1…9`, `0`.
    ///
    /// This is layout-independent: the key in that position is the key, whatever
    /// character it types.
    const NUMBER_ROW_KEY_CODES: [u16; 10] = [18, 19, 20, 21, 23, 22, 26, 28, 25, 29];

    /// The slot the digit opens, or `None` when the digit is not a slot.
    pub fn index_for_digit(digit: char) -> Option<usize> {
        Self::DIGITS.iter().position(|d| *d == digit)
    }

    /// The slot a physical number-row key opens, or `None` when that key is not
    /// a slot.
    pub fn index_for_key_code(key_code: u16) -> Option<usize> {
        Self::NUMBER_ROW_KEY_CODES
            .iter()
            .position(|code| *code == key_code)
    }

    /// The digit the slot shows, or `None` when there is no such slot.
    pub fn digit_at(index: isize) -> Option<char> {
        if index < 0 {
            return None;
        }
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::DIGITS.get(index).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream: "Ten digit keys is the ceiling: there is no ⌘10, so the
    /// eleventh favorite gets no chord."
    #[test]
    fn there_are_ten_slots_and_no_digit_repeats() {
        assert_eq!(FavoriteSlots::DIGITS.len(), 10);
        let mut seen: Vec<char> = FavoriteSlots::DIGITS.to_vec();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), FavoriteSlots::DIGITS.len());
    }

    #[test]
    fn a_digit_names_its_slot() {
        assert_eq!(FavoriteSlots::index_for_digit('1'), Some(0));
        assert_eq!(FavoriteSlots::index_for_digit('9'), Some(8));
        // The tenth slot is `0`, not the first.
        assert_eq!(FavoriteSlots::index_for_digit('0'), Some(9));
        assert_eq!(FavoriteSlots::index_for_digit('a'), None);
    }

    /// Upstream: "The numbers follow key positions, so they work the same on any
    /// keyboard layout."
    #[test]
    fn a_physical_number_row_key_names_its_slot() {
        let key_codes = [18, 19, 20, 21, 23, 22, 26, 28, 25, 29];
        for (index, key_code) in key_codes.iter().enumerate() {
            assert_eq!(
                FavoriteSlots::index_for_key_code(*key_code),
                Some(index),
                "key code {key_code} should open slot {index}"
            );
        }
        assert_eq!(FavoriteSlots::index_for_key_code(0), None);
    }

    #[test]
    fn a_slot_shows_its_digit() {
        assert_eq!(FavoriteSlots::digit_at(0), Some('1'));
        assert_eq!(FavoriteSlots::digit_at(9), Some('0'));
        // The eleventh favorite has no chord, so it shows nothing.
        assert_eq!(FavoriteSlots::digit_at(10), None);
        assert_eq!(FavoriteSlots::digit_at(-1), None);
    }

    /// Upstream: "The row overlay and the key handler must agree, or a row
    /// advertises another's chord."
    #[test]
    fn a_slot_and_its_digit_round_trip() {
        for index in 0..FavoriteSlots::DIGITS.len() {
            let digit = FavoriteSlots::digit_at(index as isize).expect("a slot has a digit");
            assert_eq!(FavoriteSlots::index_for_digit(digit), Some(index));
        }
    }
}
