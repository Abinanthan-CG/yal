//! Clipboard access.
//!
//! Reading the clipboard is best-effort: another process may hold it open, or
//! it may hold an image rather than text. Both cases are reported, never
//! silently turned into an empty string.

/// A failure while reading or writing the clipboard.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("the clipboard could not be accessed: {0}")]
    Unavailable(String),

    #[error("the clipboard does not hold text")]
    NotText,

    #[error("the clipboard could not be written: {0}")]
    Write(String),
}

/// The system clipboard.
pub struct Clipboard {
    inner: arboard::Clipboard,
}

impl Clipboard {
    /// Take control of the system clipboard.
    pub fn new() -> Result<Self, ClipboardError> {
        let inner = arboard::Clipboard::new()
            .map_err(|err| ClipboardError::Unavailable(err.to_string()))?;
        Ok(Self { inner })
    }

    /// The text currently on the clipboard.
    pub fn read_text(&mut self) -> Result<String, ClipboardError> {
        self.inner
            .get_text()
            .map_err(|_| ClipboardError::NotText)
    }

    /// The text on the clipboard, or `None` when it does not hold text.
    pub fn text(&mut self) -> Option<String> {
        self.read_text().ok()
    }

    /// Replace the clipboard contents.
    pub fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.inner
            .set_text(text.to_string())
            .map_err(|err| ClipboardError::Write(err.to_string()))
    }

    /// Put an image on the clipboard.
    ///
    /// The panel bytes are plain RGBA, `width` by `height`; arboard works
    /// out the rest. A region capture pastes into a chat or editor as a
    /// picture, which is where most captures go.
    pub fn write_image(
        &mut self,
        rgba: &[u8],
        width: usize,
        height: usize,
    ) -> Result<(), ClipboardError> {
        if width == 0 || height == 0 {
            return Err(ClipboardError::Write(
                "an empty picture cannot be put on the clipboard".into(),
            ));
        }
        if rgba.len() != width * height * 4 {
            return Err(ClipboardError::Write(format!(
                "the picture bytes do not match {}x{} RGBA",
                width, height
            )));
        }
        let data = arboard::ImageData {
            width,
            height,
            bytes: std::borrow::Cow::Borrowed(rgba),
        };
        self.inner
            .set_image(data)
            .map_err(|err| ClipboardError::Write(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_then_reading_round_trips() {
        // The clipboard is a shared resource: a history keeper — another
        // launcher running on this machine, for one — may hold it open
        // for a few milliseconds as it watches for changes. A read that
        // lands inside that window waits briefly; what is written must
        // still be what is read.
        let Ok(mut clipboard) = Clipboard::new() else {
            return;
        };
        if clipboard.write_text("yal clipboard test").is_err() {
            return;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            if clipboard.text().as_deref() == Some("yal clipboard test") {
                return;
            }
            if std::time::Instant::now() >= deadline {
                panic!("the text written did not survive on the clipboard");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn an_ill_shaped_picture_is_refused_before_the_clipboard_sees_it() {
        let Ok(mut clipboard) = Clipboard::new() else {
            return;
        };
        // The refusals come first, and are unconditional: neither shape
        // ever reaches the clipboard.
        let one_by_one = vec![90, 120, 200, 255];
        assert!(
            clipboard.write_image(&one_by_one, 2, 2).is_err(),
            "four bytes are not enough for a 2x2 picture"
        );
        assert!(
            clipboard.write_image(&[], 1, 1).is_err(),
            "no bytes at all are not a picture"
        );
        // A well-shaped write may be refused while another process holds
        // the clipboard as it watches for changes; briefly retried, then
        // reported rather than failed — the clipboard is not this test's
        // subject, the refusals above are.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        loop {
            if clipboard.write_image(&one_by_one, 1, 1).is_ok() {
                return;
            }
            if std::time::Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
