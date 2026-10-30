//! The emoji dataset.
//!
//! The table is generated from Unicode's emoji-test.txt and the CLDR
//! annotations — see `emoji_data.rs` for provenance and `gen/` for the
//! generator. Each entry carries a name and a set of keywords, so the
//! picker finds an emoji by what it means ("happy") as well as by what
//! it is called ("grinning face").

mod emoji_data;

/// One emoji.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emoji {
    /// The emoji character.
    pub character: String,
    /// The canonical name.
    pub name: String,
    /// Alternative search terms.
    pub keywords: Vec<String>,
}

/// The whole dataset.
pub fn all() -> Vec<Emoji> {
    emoji_data::EMOJI
        .iter()
        .map(|(character, name, keywords)| Emoji {
            character: (*character).to_string(),
            name: (*name).to_string(),
            keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
        })
        .collect()
}

/// Search the dataset by name and keyword.
///
/// An empty query returns the first `limit` entries, which is what the
/// palette shows before the user types anything.
pub fn search(query: &str, limit: usize) -> Vec<Emoji> {
    let needle = query.trim().to_lowercase();
    let all = all();

    if needle.is_empty() {
        return all.into_iter().take(limit).collect();
    }

    let mut scored: Vec<(u32, Emoji)> = all
        .into_iter()
        .filter_map(|emoji| {
            let name = emoji.name.to_lowercase();
            let score = if name == needle {
                100
            } else if name.starts_with(&needle) {
                80
            } else if name.contains(&needle) {
                60
            } else if emoji
                .keywords
                .iter()
                .any(|keyword| keyword.contains(&needle))
            {
                40
            } else {
                return None;
            };
            Some((score, emoji))
        })
        .collect();

    scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, emoji)| emoji)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_is_full() {
        // The Unicode 16.0 fully-qualified catalogue holds ~1,900 entries
        // once components and skin-tone sequences are dropped.
        assert!(all().len() > 1_500, "dataset holds {}", all().len());
    }

    #[test]
    fn names_are_unique() {
        let mut names: Vec<String> = all().into_iter().map(|e| e.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "emoji names must be unique");
    }

    #[test]
    fn search_finds_by_name() {
        let found = search("rocket", 5);
        assert!(!found.is_empty());
        assert_eq!(found[0].name, "rocket");
        assert_eq!(found[0].character, "🚀");
    }

    #[test]
    fn search_finds_by_keyword() {
        let found = search("pet", 10);
        assert!(
            found.iter().any(|e| e.name == "cat" || e.name == "cat face"),
            "a cat emoji should be found by the pet keyword"
        );
    }

    #[test]
    fn search_finds_sequences_and_flags() {
        let found = search("heart hands", 5);
        assert!(!found.is_empty(), "ZWJ sequences must be in the dataset");
        let flags = search("germany", 5);
        assert!(
            flags
                .iter()
                .any(|e| e.name.to_lowercase().contains("flag") && e.name.to_lowercase().contains("germany")),
            "flags must be in the dataset"
        );
    }

    #[test]
    fn empty_query_returns_the_head_of_the_dataset() {
        let found = search("", 5);
        assert_eq!(found.len(), 5);
    }

    #[test]
    fn unmatched_query_returns_nothing() {
        assert!(search("zzzzqqq", 10).is_empty());
    }
}
