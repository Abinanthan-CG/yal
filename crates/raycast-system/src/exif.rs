//! EXIF metadata for photographs.
//!
//! A photograph in the file results gains its preview metadata: the camera
//! that took it, when, and the exposure settings. The metadata sits at the
//! front of JPEG files; other formats rarely carry EXIF on Windows, so the
//! reader honestly reports "no metadata" rather than pretending the file
//! has none.

/// The metadata a photograph carries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PhotoMetadata {
    /// The maker of the camera, e.g. `"Canon"`.
    pub make: Option<String>,
    /// The camera model, e.g. `"EOS R5"`.
    pub model: Option<String>,
    /// When the picture was taken, as recorded in the file, e.g.
    /// `"2025:03:14 09:26:53"`.
    pub taken: Option<String>,
    /// The recorded pixel size, when the file states it.
    pub dimensions: Option<(u32, u32)>,
    /// The sensor sensitivity, e.g. `"800"`.
    pub iso: Option<String>,
    /// The exposure time, e.g. `"1/250 s"`.
    pub exposure: Option<String>,
    /// The aperture, e.g. `"f/2.8"`.
    pub aperture: Option<String>,
    /// The focal length, e.g. `"50 mm"`.
    pub focal_length: Option<String>,
}

impl PhotoMetadata {
    /// The non-empty metadata as label-value lines, in the order the
    /// preview shows them.
    pub fn lines(&self) -> Vec<(String, String)> {
        let mut lines = Vec::new();
        let camera = match (&self.make, &self.model) {
            (Some(make), Some(model)) => Some(format!("{make} {model}")),
            (only, None) => only.clone(),
            (None, only) => only.clone(),
        };
        if let Some(camera) = camera {
            lines.push(("Camera".into(), camera));
        }
        if let Some(taken) = &self.taken {
            lines.push(("Taken".into(), taken.clone()));
        }
        if let Some((width, height)) = self.dimensions {
            lines.push(("Dimensions".into(), format!("{width} × {height}")));
        }
        for (label, value) in [
            ("ISO", &self.iso),
            ("Exposure", &self.exposure),
            ("Aperture", &self.aperture),
            ("Focal length", &self.focal_length),
        ] {
            if let Some(value) = value {
                lines.push((label.into(), value.clone()));
            }
        }
        lines
    }
}

/// Read the metadata out of a photograph.
///
/// `None` when the file is not a photograph Windows can read this way, or
/// when the file carries no metadata — both are honest absences.
pub fn read(path: &std::path::Path) -> Option<PhotoMetadata> {
    let is_jpeg = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
        });
    if !is_jpeg {
        return None;
    }

    let file = std::fs::File::open(path).ok()?;
    let exif = exif::Reader::new()
        .read_from_container(&mut std::io::BufReader::new(file))
        .ok()?;

    fn text(exif: &exif::Exif, tag: exif::Tag, primary: exif::In) -> Option<String> {
        exif.get_field(tag, primary)
            .map(|field| field.value.display_as(field.tag).to_string())
    }

    let width = text(&exif, exif::Tag::ImageWidth, exif::In::PRIMARY)
        .and_then(|value| value.parse::<u32>().ok());
    let height = text(&exif, exif::Tag::ImageLength, exif::In::PRIMARY)
        .and_then(|value| value.parse::<u32>().ok());
    let dimensions = width.zip(height);

    Some(PhotoMetadata {
        make: text(&exif, exif::Tag::Make, exif::In::PRIMARY),
        model: text(&exif, exif::Tag::Model, exif::In::PRIMARY),
        taken: text(&exif, exif::Tag::DateTimeOriginal, exif::In::PRIMARY),
        dimensions,
        iso: text(&exif, exif::Tag::PhotographicSensitivity, exif::In::PRIMARY),
        exposure: text(&exif, exif::Tag::ExposureTime, exif::In::PRIMARY),
        aperture: text(&exif, exif::Tag::FNumber, exif::In::PRIMARY),
        focal_length: text(&exif, exif::Tag::FocalLength, exif::In::PRIMARY),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_jpeg_files_are_photographs_to_the_reader() {
        // The extension decides before the file is opened, so a path to no
        // file at all is enough for the shape of the decision.
        assert!(read(std::path::Path::new("C:/nowhere/photo.txt")).is_none());
        assert!(read(std::path::Path::new("C:/nowhere/photo.png")).is_none());
        assert!(read(std::path::Path::new("C:/nowhere/photo.JPG")).is_none());
    }

    #[test]
    fn metadata_lines_come_out_in_the_preview_order() {
        let metadata = PhotoMetadata {
            make: Some("Canon".into()),
            model: None,
            taken: Some("2024:12:17 09:50:51".into()),
            dimensions: Some((604, 300)),
            iso: Some("800".into()),
            exposure: None,
            aperture: Some("f/2.8".into()),
            focal_length: None,
        };

        let lines = metadata.lines();
        assert_eq!(lines[0], ("Camera".into(), "Canon".into()));
        assert_eq!(lines[1], ("Taken".into(), "2024:12:17 09:50:51".into()));
        assert_eq!(lines[2], ("Dimensions".into(), "604 × 300".into()));
        assert_eq!(lines[3], ("ISO".into(), "800".into()));
        // Fields the file does not carry are left out, quietly.
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn empty_metadata_makes_no_lines() {
        assert!(PhotoMetadata::default().lines().is_empty());
    }
}

