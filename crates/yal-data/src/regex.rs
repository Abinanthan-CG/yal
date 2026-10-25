//! Regular expression matching.
//!
//! Backs the regex command: the user types a pattern, and the launcher
//! reports what it matches, including capture groups.

/// One match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexMatch {
    /// The byte range of the whole match in the subject.
    pub start: usize,
    pub end: usize,
    /// The matched text.
    pub text: String,
    /// The capture groups, in order. A group that did not participate is
    /// `None`.
    pub groups: Vec<Option<String>>,
}

/// The result of matching a pattern against a subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegexResult {
    /// Every match, in order.
    pub matches: Vec<RegexMatch>,
    /// The pattern's error, when it did not compile.
    pub error: Option<String>,
}

/// Match a pattern against a subject.
///
/// A pattern that does not compile is reported as an error rather than as an
/// empty result, because "your pattern is wrong" and "your pattern matched
/// nothing" are different answers.
pub fn match_pattern(pattern: &str, subject: &str) -> RegexResult {
    let compiled = match regex::Regex::new(pattern) {
        Ok(compiled) => compiled,
        Err(error) => {
            return RegexResult {
                matches: Vec::new(),
                error: Some(error.to_string()),
            };
        }
    };

    let matches = compiled
        .captures_iter(subject)
        .map(|captures| {
            let whole = captures.get(0).expect("group zero always participates");
            let groups = captures
                .iter()
                .skip(1)
                .map(|group| group.map(|m| m.as_str().to_string()))
                .collect();
            RegexMatch {
                start: whole.start(),
                end: whole.end(),
                text: whole.as_str().to_string(),
                groups,
            }
        })
        .collect();

    RegexResult {
        matches,
        error: None,
    }
}

/// A summary of a match, for display in the palette.
pub fn summarise(result: &RegexResult) -> String {
    if let Some(error) = &result.error {
        return error.clone();
    }
    match result.matches.len() {
        0 => "No matches".to_string(),
        1 => "1 match".to_string(),
        count => format!("{count} matches"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_simple_pattern() {
        let result = match_pattern(r"\d+", "a1 b22 c333");
        assert!(result.error.is_none());
        assert_eq!(result.matches.len(), 3);
        assert_eq!(result.matches[1].text, "22");
    }

    #[test]
    fn reports_capture_groups() {
        let result = match_pattern(r"(\w+)@(\w+)", "user@example");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(
            result.matches[0].groups,
            vec![Some("user".to_string()), Some("example".to_string())]
        );
    }

    #[test]
    fn an_unmatched_group_is_none() {
        let result = match_pattern(r"a(x)?b", "ab");
        assert_eq!(result.matches[0].groups, vec![None]);
    }

    #[test]
    fn a_bad_pattern_reports_an_error() {
        let result = match_pattern("([", "text");
        assert!(result.error.is_some());
        assert!(result.matches.is_empty());
    }

    #[test]
    fn summarises_the_result() {
        assert_eq!(summarise(&match_pattern(r"\d", "a1")), "1 match");
        assert_eq!(summarise(&match_pattern(r"\d", "a1b2")), "2 matches");
        assert_eq!(summarise(&match_pattern(r"\d", "abc")), "No matches");
        assert!(summarise(&match_pattern("([", "abc")).contains("error"));
    }
}
