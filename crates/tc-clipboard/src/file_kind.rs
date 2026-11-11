//! What a referenced file is, for the type row and the tile a thumbnail has
//! not filled yet.
//!
//! Upstream oracle: `Features/Clipboard/Model/ClipboardFileKind.swift`.

/// The kind of file an entry refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipboardFileKind {
    /// An image file, recognised by its extension.
    Image,
    /// A movie file, recognised by its extension.
    Movie,
    /// An audio file, recognised by its extension.
    Audio,
    /// A PDF, recognised by its extension.
    Pdf,
    /// A directory.
    Folder,
    /// Anything else.
    Other,
}

impl ClipboardFileKind {
    /// Resolve the kind from the path.
    ///
    /// Upstream `ClipboardFileKind.of(path:isDirectory:)`: the extension decides,
    /// never the disk, so a vanished file still classifies. A directory is
    /// always a folder, whatever it is called.
    pub fn of(path: &str, is_directory: bool) -> ClipboardFileKind {
        if is_directory {
            return ClipboardFileKind::Folder;
        }
        match extension_of(path) {
            Some(ext) => classify_extension(&ext),
            None => ClipboardFileKind::Other,
        }
    }

    /// Whether the file can be played rather than merely previewed.
    ///
    /// Upstream `ClipboardFileKind.isPlayable`.
    pub fn is_playable(self) -> bool {
        matches!(self, ClipboardFileKind::Movie | ClipboardFileKind::Audio)
    }
}

/// The lowercased extension of a path, without the dot.
fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let (stem, extension) = name.rsplit_once('.')?;
    // A leading dot means a hidden file, not an extension: `.bashrc` has none.
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

/// Map an extension to the kind upstream's `UTType` conformance resolves to.
///
/// Upstream asks `UniformTypeIdentifiers` whether the extension conforms to
/// `.image`, `.movie`, `.audio` or `.pdf`; the extensions below are the ones
/// those types cover that a clipboard can plausibly carry.
fn classify_extension(extension: &str) -> ClipboardFileKind {
    match extension {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "heif" | "webp"
        | "ico" | "svg" | "avif" | "psd" | "raw" | "dng" | "cr2" | "nef" | "arw" => {
            ClipboardFileKind::Image
        }
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm" | "mpg" | "mpeg" | "wmv" | "flv" | "3gp"
        | "mts" | "m2ts" | "vob" => ClipboardFileKind::Movie,
        "m4a" | "mp3" | "wav" | "aiff" | "aif" | "flac" | "ogg" | "oga" | "m4b" | "aac" | "wma"
        | "opus" | "mid" | "midi" => ClipboardFileKind::Audio,
        "pdf" => ClipboardFileKind::Pdf,
        _ => ClipboardFileKind::Other,
    }
}
