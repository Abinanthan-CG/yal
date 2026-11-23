//! Portable backup export and verified restore for YAL user data.
//!
//! Captures settings, learned ranking, favorites, quicklinks, emoji pins &
//! frequency, calculator history, custom commands, snippets (`snippets/*.md`),
//! and quick notes (`notes/*.md`) into a single versioned `.yalbackup.json`
//! envelope sealed with a SHA-256 integrity digest.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Current backup envelope format tag.
pub const BACKUP_SCHEMA: &str = "yal-backup/v1";

/// Top-level JSON config files included in a portable backup.
const ROOT_FILES: &[&str] = &[
    "settings.json",
    "launcher-ranking.json",
    "favorites.json",
    "quicklinks.json",
    "emoji-pins.json",
    "emoji-frequency.json",
    "calc-history.json",
    "commands.json",
];

/// Subdirectories of Markdown documents included in a portable backup.
const MARKDOWN_DIRS: &[&str] = &["snippets", "notes"];

/// What can go wrong during export or restore.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("backup I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("backup JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported backup schema `{0}`")]
    UnsupportedSchema(String),
    #[error("backup integrity check failed (checksum mismatch)")]
    ChecksumMismatch,
    #[error("backup contains an unsafe relative path `{0}`")]
    UnsafePath(String),
}

/// Inner versioned payload whose canonical JSON is hashed with SHA-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupPayload {
    pub created_at_secs: i64,
    pub files: BTreeMap<String, String>,
}

/// Complete sealed `.yalbackup.json` envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupArchive {
    pub schema: String,
    pub sha256: String,
    pub payload: BackupPayload,
}

/// Summary of an export or restore operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupSummary {
    pub files_processed: usize,
}

impl BackupArchive {
    /// Collect every portable configuration and Markdown file from `data_dir`.
    pub fn collect(data_dir: &Path) -> Result<Self, BackupError> {
        let mut files = BTreeMap::new();

        for name in ROOT_FILES {
            let path = data_dir.join(name);
            if path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    files.insert((*name).to_string(), content);
                }
            }
        }

        for dir_name in MARKDOWN_DIRS {
            let dir = data_dir.join(dir_name);
            if !dir.is_dir() {
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let is_md = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("md"));
                if !is_md {
                    continue;
                }
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        files.insert(format!("{dir_name}/{file_name}"), content);
                    }
                }
            }
        }

        let created_at_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let payload = BackupPayload {
            created_at_secs,
            files,
        };
        let sha256 = payload_digest(&payload)?;
        Ok(Self {
            schema: BACKUP_SCHEMA.to_string(),
            sha256,
            payload,
        })
    }

    /// Verify schema version, SHA-256 digest, and safe relative file paths.
    pub fn verify(&self) -> Result<(), BackupError> {
        if self.schema != BACKUP_SCHEMA {
            return Err(BackupError::UnsupportedSchema(self.schema.clone()));
        }
        let expected = payload_digest(&self.payload)?;
        if !self.sha256.eq_ignore_ascii_case(&expected) {
            return Err(BackupError::ChecksumMismatch);
        }
        for rel in self.payload.files.keys() {
            validate_safe_rel_path(rel)?;
        }
        Ok(())
    }
}

/// Export all YAL user data under `data_dir` to a verified `.yalbackup.json` file.
pub fn export_backup(data_dir: &Path, output_path: &Path) -> Result<BackupSummary, BackupError> {
    let archive = BackupArchive::collect(data_dir)?;
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let json = serde_json::to_string_pretty(&archive)?;
    std::fs::write(output_path, json)?;
    Ok(BackupSummary {
        files_processed: archive.payload.files.len(),
    })
}

/// Verify and restore a `.yalbackup.json` file into `data_dir`.
pub fn import_backup(backup_path: &Path, data_dir: &Path) -> Result<BackupSummary, BackupError> {
    let raw = std::fs::read_to_string(backup_path)?;
    let archive: BackupArchive = serde_json::from_str(&raw)?;
    archive.verify()?;

    std::fs::create_dir_all(data_dir)?;
    for (rel, content) in &archive.payload.files {
        validate_safe_rel_path(rel)?;
        let dest = data_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic write via sibling temp file in the destination directory.
        let tmp = dest.with_extension("yal-restore-tmp");
        std::fs::write(&tmp, content)?;
        if let Err(err) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_file(&tmp);
            return Err(BackupError::Io(err));
        }
    }
    Ok(BackupSummary {
        files_processed: archive.payload.files.len(),
    })
}

fn payload_digest(payload: &BackupPayload) -> Result<String, BackupError> {
    let canonical = serde_json::to_vec(payload)?;
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    Ok(hex::encode(hasher.finalize()))
}

/// Strictly permit only known root JSON files or `snippets/<name>.md` / `notes/<name>.md`.
fn validate_safe_rel_path(rel: &str) -> Result<(), BackupError> {
    if rel.is_empty() || rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        return Err(BackupError::UnsafePath(rel.to_string()));
    }
    let path = Path::new(rel);
    if path.is_absolute() {
        return Err(BackupError::UnsafePath(rel.to_string()));
    }
    let parts: Vec<&str> = path
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| BackupError::UnsafePath(rel.to_string()))?;

    match parts.as_slice() {
        [file] if ROOT_FILES.contains(file) => Ok(()),
        [dir, file]
            if MARKDOWN_DIRS.contains(dir)
                && !file.is_empty()
                && !file.starts_with('.')
                && file.to_ascii_lowercase().ends_with(".md") =>
        {
            Ok(())
        }
        _ => Err(BackupError::UnsafePath(rel.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "yal-backup-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip_export_and_import_preserves_settings_snippets_and_notes() {
        let src = temp_dir("src");
        let dst = temp_dir("dst");
        let archive_file = temp_dir("out").join("backup.yalbackup.json");

        std::fs::write(
            src.join("settings.json"),
            r#"{"palette.appearance":"Dark"}"#,
        )
        .unwrap();
        std::fs::write(src.join("favorites.json"), r#"["app:notepad"]"#).unwrap();
        std::fs::create_dir_all(src.join("snippets")).unwrap();
        std::fs::write(src.join("snippets").join("sig.md"), "Best regards,\nYAL").unwrap();
        std::fs::create_dir_all(src.join("notes")).unwrap();
        std::fs::write(src.join("notes").join("ideas.md"), "# Ideas\nShip fast!").unwrap();
        // Unrelated file must NOT be swept into the backup.
        std::fs::write(src.join("secret.key"), "do-not-export").unwrap();

        let exported = export_backup(&src, &archive_file).expect("export succeeds");
        assert_eq!(exported.files_processed, 4);

        // Pre-populate destination to prove restore overwrites atomically on Windows.
        std::fs::write(
            dst.join("settings.json"),
            r#"{"palette.appearance":"Light"}"#,
        )
        .unwrap();

        let imported = import_backup(&archive_file, &dst).expect("import succeeds");
        assert_eq!(imported.files_processed, 4);

        assert_eq!(
            std::fs::read_to_string(dst.join("settings.json")).unwrap(),
            r#"{"palette.appearance":"Dark"}"#
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("snippets/sig.md")).unwrap(),
            "Best regards,\nYAL"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("notes/ideas.md")).unwrap(),
            "# Ideas\nShip fast!"
        );
        assert!(!dst.join("secret.key").exists());

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(archive_file.parent().unwrap());
    }

    #[test]
    fn rejects_tampered_checksum() {
        let src = temp_dir("tamper-src");
        let dst = temp_dir("tamper-dst");
        let archive_file = temp_dir("tamper-out").join("backup.yalbackup.json");

        std::fs::write(src.join("settings.json"), r#"{"a":"1"}"#).unwrap();
        export_backup(&src, &archive_file).unwrap();

        let original = std::fs::read_to_string(&archive_file).unwrap();
        let tampered = original.replace(r#"\"1\""#, r#"\"2\""#);
        assert_ne!(original, tampered);
        std::fs::write(&archive_file, tampered).unwrap();

        let err = import_backup(&archive_file, &dst).unwrap_err();
        assert!(matches!(err, BackupError::ChecksumMismatch));

        let _ = std::fs::remove_dir_all(&src);
        let _ = std::fs::remove_dir_all(&dst);
        let _ = std::fs::remove_dir_all(archive_file.parent().unwrap());
    }

    #[test]
    fn rejects_path_traversal_entries_even_with_valid_checksum() {
        let mut files = BTreeMap::new();
        files.insert("../evil.txt".to_string(), "pwned".to_string());
        let payload = BackupPayload {
            created_at_secs: 0,
            files,
        };
        let sha256 = payload_digest(&payload).unwrap();
        let archive = BackupArchive {
            schema: BACKUP_SCHEMA.to_string(),
            sha256,
            payload,
        };
        assert!(matches!(archive.verify(), Err(BackupError::UnsafePath(_))));
    }
}
