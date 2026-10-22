//! QR code generation.
//!
//! A QR code is produced as a PNG so that the palette can show it as an
//! image and the command line can write it to a file.

/// The rendered QR code.
pub struct QrCode {
    /// The PNG bytes.
    pub png: Vec<u8>,
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
}

/// A failure while generating a QR code.
#[derive(Debug, thiserror::Error)]
pub enum QrError {
    #[error("the text is too long to encode: {0} characters")]
    TooLong(usize),

    #[error("the QR code could not be rendered: {0}")]
    Render(String),
}

/// The largest payload a single QR code can carry.
const MAX_LENGTH: usize = 2_953;

/// Generate a QR code for some text.
///
/// `scale` is the number of pixels per module.
pub fn generate(text: &str, scale: u32) -> Result<QrCode, QrError> {
    if text.chars().count() > MAX_LENGTH {
        return Err(QrError::TooLong(text.chars().count()));
    }

    let code = qrcode::QrCode::with_error_correction_level(
        text,
        qrcode::EcLevel::M,
    )
    .map_err(|err| QrError::Render(err.to_string()))?;

    let image = code
        .render::<image::Luma<u8>>()
        .quiet_zone(true)
        .min_dimensions(64 * scale, 64 * scale)
        .build();

    let width = image.width();
    let height = image.height();

    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|err| QrError::Render(err.to_string()))?;

    Ok(QrCode {
        png,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_png() {
        let code = generate("https://example.com", 4).unwrap();
        assert!(code.width > 0);
        assert!(code.height > 0);
        // A PNG starts with its signature.
        assert_eq!(&code.png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn rejects_text_that_is_too_long() {
        let long = "x".repeat(MAX_LENGTH + 1);
        assert!(matches!(generate(&long, 4), Err(QrError::TooLong(_))));
    }

    #[test]
    fn different_text_produces_different_codes() {
        let first = generate("one", 4).unwrap();
        let second = generate("two", 4).unwrap();
        assert_ne!(first.png, second.png);
    }
}
