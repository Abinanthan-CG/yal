//! The search scopes and the ignore patterns.
//!
//! Upstream indexes the folders the user picks, and never indexes anything
//! else: an empty scope list indexes nothing, and there is no fall back to the
//! home folder. The user also writes rules that keep whole folders out of
//! the index.
//!
//! The three kinds of rule are upstream's:
//!
//! | Pattern | Checked against | Example |
//! | --- | --- | --- |
//! | a plain word | any folder or name | `node_modules` |
//! | one with `*` `?` `[` | any folder or name | `*.tmp` |
//! | one containing `/`, or starting `~/` | the whole path | `~/Movies` |
//!
//! Matching is case-insensitive, and `*` matches `/` as well, so a
//! `**/…/**` pattern works as it is written. A path rule that starts with `~`
//! is matched against the home folder, the same way a scope is stored.

use std::path::{Path, PathBuf};

/// The name the home folder is written as in a stored scope and in a rule.
///
/// A scope saved as `~/Documents` still points at the home folder on
/// another machine, which is what makes a backup portable.
pub const HOME_PREFIX: &str = "~";

/// One indexed folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The folder to index.
    pub path: PathBuf,
}

impl Scope {
    /// A scope for a folder.
    pub fn new(path: impl Into<PathBuf>) -> Scope {
        Scope { path: path.into() }
    }
}

/// The folders to index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeList {
    scopes: Vec<Scope>,
}

impl ScopeList {
    /// A list of scopes. An empty list indexes nothing.
    pub fn new(scopes: Vec<PathBuf>) -> ScopeList {
        ScopeList {
            scopes: scopes.into_iter().map(Scope::new).collect(),
        }
    }

    /// Whether the list is empty. An empty list indexes nothing at all.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// How many scopes there are.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// The scopes themselves.
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// The folders to index.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.scopes.iter().map(|scope| scope.path.clone()).collect()
    }

    /// The list as it is written to the settings file, with the home folder
    /// written as `~`.
    pub fn to_stored(&self, home: &Path) -> Vec<String> {
        self.scopes
            .iter()
            .map(|scope| stored_path(&scope.path, home))
            .collect()
    }

    /// The list read back from the settings file, with `~` expanded against
    /// the given home folder.
    pub fn from_stored(stored: &[String], home: &Path) -> ScopeList {
        ScopeList::new(
            stored
                .iter()
                .map(|text| expanded_path(text, home))
                .collect(),
        )
    }
}

/// A path written the way the settings file holds it: the home folder as `~`.
pub fn stored_path(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => format!("{HOME_PREFIX}/"),
        Ok(rest) => {
            let rest = rest.to_string_lossy().replace('\\', "/");
            format!("{HOME_PREFIX}/{rest}")
        }
        Err(_) => path.to_string_lossy().replace('\\', "/"),
    }
}

/// A path read back from the settings file: `~` expanded against the home
/// folder.
pub fn expanded_path(text: &str, home: &Path) -> PathBuf {
    let trimmed = text.trim();
    if trimmed == HOME_PREFIX {
        return home.to_path_buf();
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return home.join(rest.replace('/', "\\"));
    }
    PathBuf::from(trimmed)
}

/// What kind of rule a pattern is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnorePatternKind {
    /// A plain word: it matches any folder or name that contains it.
    Word,
    /// A pattern with a `*`, a `?` or a `[` in it.
    Glob,
    /// A rule that contains a `/`: it matches the whole path.
    Path,
}

/// One rule that keeps things out of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnorePattern {
    /// The kind of rule.
    pub kind: IgnorePatternKind,
    /// The rule itself.
    pub text: String,
}

impl IgnorePattern {
    /// Read a rule from what the user typed, and work out what kind it is.
    ///
    /// A pattern with a wildcard in it is a pattern even when it also has a
    /// `/` in it: upstream's `**/…/**` rules work as they are written, and
    /// `*` matches `/` as well.
    pub fn parse(text: &str) -> IgnorePattern {
        let trimmed = text.trim().to_string();
        let kind = if trimmed.contains('*') || trimmed.contains('?') || trimmed.contains('[') {
            IgnorePatternKind::Glob
        } else if trimmed.contains('/') {
            IgnorePatternKind::Path
        } else {
            IgnorePatternKind::Word
        };
        IgnorePattern {
            kind,
            text: trimmed,
        }
    }

    /// Whether the rule keeps a folder or a name out of the index.
    ///
    /// A path rule is matched against the whole path with `/` separators, so
    /// a rule written the way a person writes it works on Windows too. A rule
    /// that starts with `~` is matched against the home folder.
    pub fn matches(&self, name: &str, path: &Path, home: &Path) -> bool {
        if self.text.is_empty() {
            return false;
        }
        match self.kind {
            IgnorePatternKind::Word => name.to_lowercase().contains(&self.text.to_lowercase()),
            IgnorePatternKind::Glob => {
                if self.text.contains('/') {
                    glob_matches(&self.text.to_lowercase(), &path_text(path, home))
                } else {
                    glob_matches(&self.text.to_lowercase(), &name.to_lowercase())
                }
            }
            IgnorePatternKind::Path => {
                let rule = self.text.replace('\\', "/").to_lowercase();
                let rule = if let Some(rest) = rule.strip_prefix("~/") {
                    format!("{}/{}", path_text(home, home).trim_end_matches('/'), rest)
                } else {
                    rule
                };
                path_text(path, home).contains(&rule)
            }
        }
    }
}

/// A path written with `/` separators and in lower case.
fn path_text(path: &Path, home: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/").to_lowercase();
    if let Ok(rest) = path.strip_prefix(home) {
        let rest = rest.to_string_lossy().replace('\\', "/").to_lowercase();
        if rest.is_empty() {
            return format!("{HOME_PREFIX}/");
        }
        return format!("{HOME_PREFIX}/{rest}");
    }
    text
}

/// Whether a name matches a shape.
///
/// `*` stands for any run of characters, `/` included; `?` for exactly one
/// character; `[abc]` and `[a-z]` for a character class.
pub fn glob_matches(shape: &str, name: &str) -> bool {
    let shape: Vec<char> = shape.chars().collect();
    let name: Vec<char> = name.chars().collect();
    glob_here(&shape, 0, &name, 0)
}

/// The matcher itself: the shape and the name, from the front.
fn glob_here(shape: &[char], shape_at: usize, name: &[char], name_at: usize) -> bool {
    if shape_at == shape.len() {
        return name_at == name.len();
    }
    match shape[shape_at] {
        '*' => {
            // `*` matches any run of characters, `/` included.
            for skip in name_at..=name.len() {
                if glob_here(shape, shape_at + 1, name, skip) {
                    return true;
                }
            }
            false
        }
        '?' => name_at < name.len() && glob_here(shape, shape_at + 1, name, name_at + 1),
        '[' => {
            let close = match shape[shape_at..].iter().position(|c| *c == ']') {
                Some(offset) if offset > 1 => shape_at + offset,
                _ => {
                    // A `[` with no class after it is a plain character.
                    return name_at < name.len()
                        && name[name_at] == '['
                        && glob_here(shape, shape_at + 1, name, name_at + 1);
                }
            };
            let class = &shape[shape_at + 1..close];
            if name_at >= name.len() {
                return false;
            }
            let found = name[name_at];
            let negated = class.first() == Some(&'!');
            let class = if negated { &class[1..] } else { class };
            let mut hit = false;
            let mut index = 0usize;
            while index < class.len() {
                if index + 2 < class.len() && class[index + 1] == '-' {
                    if found >= class[index] && found <= class[index + 2] {
                        hit = true;
                    }
                    index += 3;
                } else {
                    if found == class[index] {
                        hit = true;
                    }
                    index += 1;
                }
            }
            if hit == negated {
                return false;
            }
            glob_here(shape, close + 1, name, name_at + 1)
        }
        wanted => {
            name_at < name.len()
                && name[name_at] == wanted
                && glob_here(shape, shape_at + 1, name, name_at + 1)
        }
    }
}

/// The rules that keep things out of the index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

impl IgnoreRules {
    /// The rules the user wrote, with no built-in rules at all.
    pub fn empty() -> IgnoreRules {
        IgnoreRules {
            patterns: Vec::new(),
        }
    }

    /// The built-in rules plus the user's own.
    ///
    /// Upstream starts from a list of built-in rules - build trees, caches
    /// and the busiest parts of the system folders - and the user may remove
    /// any of them.
    pub fn with_user_patterns(user: Vec<IgnorePattern>) -> IgnoreRules {
        let mut patterns = IgnoreRules::defaults().patterns;
        patterns.extend(user);
        IgnoreRules { patterns }
    }

    /// The built-in rules.
    ///
    /// Upstream's list is macOS-shaped; these are the Windows equivalents of
    /// the same kinds of folder: build trees and dependency folders, the
    /// caches, and the busiest parts of the system folders. Nothing else is
    /// skipped - upstream indexes hidden files, and so does this port.
    pub fn defaults() -> IgnoreRules {
        let defaults = [
            // Build and dependency folders.
            "node_modules",
            ".git",
            "build",
            "DerivedData",
            "target",
            "venv",
            "__pycache__",
            // The caches.
            ".cache",
            ".npm",
            // The system folders that are never the user's own files.
            "$RECYCLE.BIN",
            "System Volume Information",
            "AppData",
            "ProgramData",
            "Windows",
            "Program Files",
            "Program Files (x86)",
        ];
        IgnoreRules {
            patterns: defaults
                .iter()
                .map(|text| IgnorePattern::parse(text))
                .collect(),
        }
    }

    /// The rules themselves.
    pub fn patterns(&self) -> &[IgnorePattern] {
        &self.patterns
    }

    /// Put the built-in rules back, keeping the user's own.
    pub fn restore_defaults(&mut self) {
        let user: Vec<IgnorePattern> = self
            .patterns
            .iter()
            .filter(|pattern| !default_texts().contains(&pattern.text))
            .cloned()
            .collect();
        self.patterns = IgnoreRules::defaults().patterns;
        self.patterns.extend(user);
    }

    /// Remove one rule by its text.
    pub fn remove(&mut self, text: &str) {
        self.patterns.retain(|pattern| pattern.text != text);
    }

    /// Whether a folder or a name is kept out of the index.
    ///
    /// A folder that matches is never entered and never recorded; a file that
    /// matches is never recorded, but the folder around it still is.
    pub fn is_ignored(&self, name: &str, path: &Path, home: &Path) -> bool {
        self.patterns
            .iter()
            .any(|pattern| pattern.matches(name, path, home))
    }
}

/// The texts of the built-in rules, so `restore_defaults` can tell them from
/// the user's own.
fn default_texts() -> Vec<String> {
    IgnoreRules::defaults()
        .patterns
        .iter()
        .map(|pattern| pattern.text.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("C:\\Users\\dabin")
    }

    #[test]
    fn an_empty_scope_list_indexes_nothing() {
        let scopes = ScopeList::new(Vec::new());
        assert!(scopes.is_empty());
        assert_eq!(scopes.len(), 0);
        assert!(scopes.paths().is_empty());
    }

    #[test]
    fn a_scope_keeps_its_folder() {
        let scopes = ScopeList::new(vec![PathBuf::from("C:\\Users\\dabin\\Documents")]);
        assert_eq!(scopes.len(), 1);
        assert_eq!(
            scopes.paths(),
            vec![PathBuf::from("C:\\Users\\dabin\\Documents")]
        );
    }

    #[test]
    fn the_home_folder_is_stored_with_a_tilde() {
        let scopes = ScopeList::new(vec![home().join("Documents"), home()]);
        let stored = scopes.to_stored(&home());
        assert_eq!(stored, vec!["~/Documents".to_string(), "~/".to_string()]);
    }

    #[test]
    fn a_stored_scope_is_read_back_against_the_home_folder() {
        let scopes =
            ScopeList::from_stored(&["~/Documents".to_string(), "~/".to_string()], &home());
        assert_eq!(scopes.paths(), vec![home().join("Documents"), home()]);
    }

    #[test]
    fn a_scope_outside_the_home_folder_is_stored_as_it_is() {
        let scopes = ScopeList::new(vec![PathBuf::from("D:\\Work")]);
        assert_eq!(scopes.to_stored(&home()), vec!["D:/Work".to_string()]);
    }

    #[test]
    fn a_plain_word_matches_any_name_that_contains_it() {
        let pattern = IgnorePattern::parse("node_modules");
        assert_eq!(pattern.kind, IgnorePatternKind::Word);
        assert!(pattern.matches("node_modules", Path::new("C:\\a\\node_modules"), &home()));
        assert!(pattern.matches("NODE_MODULES", Path::new("C:\\a\\NODE_MODULES"), &home()));
        assert!(!pattern.matches("src", Path::new("C:\\a\\src"), &home()));
    }

    #[test]
    fn a_star_matches_any_run_of_characters() {
        let pattern = IgnorePattern::parse("*.tmp");
        assert_eq!(pattern.kind, IgnorePatternKind::Glob);
        assert!(pattern.matches("scratch.tmp", Path::new("C:\\a\\scratch.tmp"), &home()));
        assert!(!pattern.matches("scratch.tmp.bak", Path::new("C:\\a\\x"), &home()));
    }

    #[test]
    fn a_star_matches_a_slash_as_well() {
        let pattern = IgnorePattern::parse("**/Movies/**");
        assert!(pattern.matches(
            "clip.mp4",
            Path::new("C:\\Users\\dabin\\Movies\\clip.mp4"),
            &home()
        ));
    }

    #[test]
    fn a_question_mark_matches_exactly_one_character() {
        let pattern = IgnorePattern::parse("a?c.txt");
        assert!(pattern.matches("abc.txt", Path::new("C:\\a\\abc.txt"), &home()));
        assert!(!pattern.matches("ac.txt", Path::new("C:\\a\\ac.txt"), &home()));
    }

    #[test]
    fn a_character_class_matches_the_characters_in_it() {
        let pattern = IgnorePattern::parse("file[0-9].txt");
        assert!(pattern.matches("file7.txt", Path::new("C:\\a\\file7.txt"), &home()));
        assert!(!pattern.matches("filex.txt", Path::new("C:\\a\\filex.txt"), &home()));
    }

    #[test]
    fn a_rule_with_a_slash_matches_the_whole_path() {
        let pattern = IgnorePattern::parse("~/Movies");
        assert_eq!(pattern.kind, IgnorePatternKind::Path);
        assert!(pattern.matches(
            "Movies",
            Path::new("C:\\Users\\dabin\\Movies\\clip.mp4"),
            &home()
        ));
        assert!(!pattern.matches(
            "Documents",
            Path::new("C:\\Users\\dabin\\Documents\\clip.mp4"),
            &home()
        ));
    }

    #[test]
    fn a_tilde_rule_does_not_match_another_machines_folder() {
        let pattern = IgnorePattern::parse("~/Movies");
        assert!(!pattern.matches("Movies", Path::new("D:\\Backup\\Movies\\clip.mp4"), &home()));
    }

    #[test]
    fn the_built_in_rules_keep_the_build_trees_out() {
        let rules = IgnoreRules::defaults();
        assert!(rules.is_ignored("node_modules", Path::new("C:\\a\\node_modules"), &home()));
        assert!(rules.is_ignored(".git", Path::new("C:\\a\\.git"), &home()));
        assert!(rules.is_ignored("DerivedData", Path::new("C:\\a\\DerivedData"), &home()));
    }

    #[test]
    fn the_built_in_rules_keep_the_system_folders_out() {
        let rules = IgnoreRules::defaults();
        assert!(rules.is_ignored("AppData", Path::new("C:\\Users\\dabin\\AppData"), &home()));
        assert!(rules.is_ignored("Program Files", Path::new("C:\\Program Files"), &home()));
        assert!(rules.is_ignored("$RECYCLE.BIN", Path::new("C:\\$RECYCLE.BIN"), &home()));
    }

    #[test]
    fn the_built_in_rules_do_not_skip_the_users_own_files() {
        let rules = IgnoreRules::defaults();
        assert!(!rules.is_ignored(
            "Documents",
            Path::new("C:\\Users\\dabin\\Documents"),
            &home()
        ));
        assert!(!rules.is_ignored(".config", Path::new("C:\\Users\\dabin\\.config"), &home()));
        assert!(!rules.is_ignored(
            "report.pdf",
            Path::new("C:\\Users\\dabin\\report.pdf"),
            &home()
        ));
    }

    #[test]
    fn a_users_own_rule_sits_next_to_the_built_in_ones() {
        let rules = IgnoreRules::with_user_patterns(vec![IgnorePattern::parse("drafts")]);
        assert!(rules.is_ignored("drafts", Path::new("C:\\a\\drafts"), &home()));
        assert!(rules.is_ignored("node_modules", Path::new("C:\\a\\node_modules"), &home()));
        assert!(rules.patterns().len() > IgnoreRules::defaults().patterns.len());
    }

    #[test]
    fn a_built_in_rule_can_be_removed() {
        let mut rules = IgnoreRules::defaults();
        let before = rules.patterns().len();
        rules.remove("node_modules");
        assert_eq!(rules.patterns().len(), before - 1);
        assert!(!rules.is_ignored("node_modules", Path::new("C:\\a\\node_modules"), &home()));
    }

    #[test]
    fn restoring_the_defaults_keeps_the_users_own_rules() {
        let mut rules = IgnoreRules::with_user_patterns(vec![IgnorePattern::parse("drafts")]);
        rules.remove("node_modules");
        rules.restore_defaults();
        assert!(rules.is_ignored("node_modules", Path::new("C:\\a\\node_modules"), &home()));
        assert!(rules.is_ignored("drafts", Path::new("C:\\a\\drafts"), &home()));
    }

    #[test]
    fn an_empty_rule_matches_nothing() {
        let pattern = IgnorePattern::parse("   ");
        assert!(!pattern.matches("anything", Path::new("C:\\a\\anything"), &home()));
    }
}
