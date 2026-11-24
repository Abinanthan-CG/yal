//! Minimal, panic-free 8-bit PNG encoding and decoding for clipboard image blobs.

use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

/// Standard 8-byte PNG file signature (`\x89PNG\r\n\x1a\n`).
pub const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

/// Maximum image width or height accepted by the encoder and decoder.
pub const MAX_PNG_DIMENSION: u32 = 4096;

/// Maximum compressed IDAT payload accepted during decoding (32 MiB).
const MAX_IDAT_BYTES: usize = 32 * 1024 * 1024;

/// Maximum decompressed scanline buffer (4096x4096 RGBA + filter byte per row).
const MAX_DECOMPRESSED_BYTES: usize =
    (MAX_PNG_DIMENSION as usize) * ((MAX_PNG_DIMENSION as usize) * 4 + 1);

/// Encode an 8-bit RGBA pixel buffer into a standard PNG byte stream.
///
/// Returns `None` if `width` or `height` is outside `1..=4096`, or if
/// `rgba.len()` does not match `width * height * 4`.
pub fn encode_png_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > MAX_PNG_DIMENSION || height > MAX_PNG_DIMENSION {
        return None;
    }
    let w = width as usize;
    let h = height as usize;
    let row_bytes = w.checked_mul(4)?;
    let expected_len = row_bytes.checked_mul(h)?;
    if rgba.len() != expected_len {
        return None;
    }

    let raw_len = row_bytes.checked_add(1)?.checked_mul(h)?;
    let mut raw = Vec::with_capacity(raw_len);
    for y in 0..h {
        raw.push(0u8); // Filter type 0 (None)
        let start = y * row_bytes;
        raw.extend_from_slice(rgba.get(start..start + row_bytes)?);
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).ok()?;
    let compressed = encoder.finish().ok()?;

    let mut out = Vec::with_capacity(8 + 25 + 12 + compressed.len() + 12);
    out.extend_from_slice(PNG_SIGNATURE);

    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8; // Bit depth
    ihdr[9] = 6; // Color type: RGBA
    ihdr[10] = 0; // Compression: deflate
    ihdr[11] = 0; // Filter: adaptive
    ihdr[12] = 0; // Interlace: none
    write_chunk(&mut out, *b"IHDR", &ihdr)?;
    write_chunk(&mut out, *b"IDAT", &compressed)?;
    write_chunk(&mut out, *b"IEND", &[])?;
    Some(out)
}

/// Decode an 8-bit non-interlaced RGBA (`color_type == 6`) or RGB
/// (`color_type == 2`) PNG into `(width, height, rgba_bytes)`.
///
/// Rejects malformed signatures, bad chunk CRCs, unsupported bit depths or
/// color types, dimensions outside `1..=4096`, unknown scanline filters, and
/// decompression bombs without panicking.
pub fn decode_png_rgba(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if png.len() < PNG_SIGNATURE.len() || &png[..8] != PNG_SIGNATURE {
        return None;
    }

    let mut pos = 8usize;
    let mut ihdr: Option<(u32, u32, u8)> = None;
    let mut idat = Vec::new();
    let mut saw_iend = false;

    while pos < png.len() {
        let header = png.get(pos..pos.checked_add(8)?)?;
        let len = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let chunk_type = [header[4], header[5], header[6], header[7]];
        let data_start = pos.checked_add(8)?;
        let data_end = data_start.checked_add(len)?;
        let crc_end = data_end.checked_add(4)?;
        let data = png.get(data_start..data_end)?;
        let crc_bytes = png.get(data_end..crc_end)?;
        let expected_crc =
            u32::from_be_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        if crc32(&chunk_type, data) != expected_crc {
            return None;
        }
        pos = crc_end;

        match &chunk_type {
            b"IHDR" => {
                if ihdr.is_some() || data.len() != 13 {
                    return None;
                }
                let width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let bit_depth = data[8];
                let color_type = data[9];
                let compression = data[10];
                let filter = data[11];
                let interlace = data[12];
                if width == 0
                    || height == 0
                    || width > MAX_PNG_DIMENSION
                    || height > MAX_PNG_DIMENSION
                    || bit_depth != 8
                    || (color_type != 6 && color_type != 2)
                    || compression != 0
                    || filter != 0
                    || interlace != 0
                {
                    return None;
                }
                ihdr = Some((width, height, color_type));
            }
            b"IDAT" => {
                ihdr?;
                if idat.len().checked_add(data.len())? > MAX_IDAT_BYTES {
                    return None;
                }
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                if !data.is_empty() {
                    return None;
                }
                saw_iend = true;
                break;
            }
            _ => {}
        }
    }

    let (width, height, color_type) = ihdr?;
    if !saw_iend || idat.is_empty() {
        return None;
    }

    let bpp = if color_type == 6 { 4usize } else { 3usize };
    let w = width as usize;
    let h = height as usize;
    let row_bytes = w.checked_mul(bpp)?;
    let stride = row_bytes.checked_add(1)?;
    let expected_raw = stride.checked_mul(h)?;
    if expected_raw > MAX_DECOMPRESSED_BYTES {
        return None;
    }

    let mut decoder =
        ZlibDecoder::new(idat.as_slice()).take((expected_raw as u64).saturating_add(1));
    let mut raw = Vec::with_capacity(expected_raw);
    decoder.read_to_end(&mut raw).ok()?;
    if raw.len() != expected_raw {
        return None;
    }

    let mut rgba = Vec::with_capacity(w.checked_mul(h)?.checked_mul(4)?);
    let mut prev_row = vec![0u8; row_bytes];
    let mut curr_row = vec![0u8; row_bytes];

    for y in 0..h {
        let row_offset = y * stride;
        let filter = *raw.get(row_offset)?;
        let src = raw.get(row_offset + 1..row_offset + stride)?;
        for i in 0..row_bytes {
            let x = src[i];
            let a = if i >= bpp { curr_row[i - bpp] } else { 0 };
            let b = prev_row[i];
            let c = if i >= bpp { prev_row[i - bpp] } else { 0 };
            curr_row[i] = match filter {
                0 => x,
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((u16::from(a) + u16::from(b)) / 2) as u8),
                4 => x.wrapping_add(paeth_predictor(a, b, c)),
                _ => return None,
            };
        }
        if color_type == 6 {
            rgba.extend_from_slice(&curr_row);
        } else {
            for pixel in curr_row.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        prev_row.copy_from_slice(&curr_row);
    }

    Some((width, height, rgba))
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: [u8; 4], data: &[u8]) -> Option<()> {
    let len = u32::try_from(data.len()).ok()?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(&chunk_type);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&chunk_type, data).to_be_bytes());
    Some(())
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a = i32::from(a);
    let b = i32::from(b);
    let c = i32::from(c);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

/// Standard IEEE 802.3 CRC-32 over a PNG chunk type and payload.
fn crc32(chunk_type: &[u8; 4], data: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for &byte in chunk_type.iter().chain(data.iter()) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_filtered_png(
        width: u32,
        height: u32,
        color_type: u8,
        pixels: &[u8],
        row_filters: &[u8],
    ) -> Vec<u8> {
        let bpp = if color_type == 6 { 4usize } else { 3usize };
        let row_bytes = (width as usize) * bpp;
        let mut raw = Vec::new();
        let mut prev_row = vec![0u8; row_bytes];
        for y in 0..(height as usize) {
            let filter = row_filters[y % row_filters.len()];
            raw.push(filter);
            let curr = &pixels[y * row_bytes..(y + 1) * row_bytes];
            for i in 0..row_bytes {
                let x = curr[i];
                let a = if i >= bpp { curr[i - bpp] } else { 0 };
                let b = prev_row[i];
                let c = if i >= bpp { prev_row[i - bpp] } else { 0 };
                let encoded = match filter {
                    0 => x,
                    1 => x.wrapping_sub(a),
                    2 => x.wrapping_sub(b),
                    3 => x.wrapping_sub(((u16::from(a) + u16::from(b)) / 2) as u8),
                    4 => x.wrapping_sub(paeth_predictor(a, b, c)),
                    _ => panic!("unsupported filter in test helper"),
                };
                raw.push(encoded);
            }
            prev_row.copy_from_slice(curr);
        }
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut out = Vec::new();
        out.extend_from_slice(PNG_SIGNATURE);
        let mut ihdr = [0u8; 13];
        ihdr[0..4].copy_from_slice(&width.to_be_bytes());
        ihdr[4..8].copy_from_slice(&height.to_be_bytes());
        ihdr[8] = 8;
        ihdr[9] = color_type;
        write_chunk(&mut out, *b"IHDR", &ihdr).unwrap();
        write_chunk(&mut out, *b"IDAT", &compressed).unwrap();
        write_chunk(&mut out, *b"IEND", &[]).unwrap();
        out
    }

    #[test]
    fn encode_and_decode_rgba_round_trip() {
        let width = 3u32;
        let height = 2u32;
        let rgba: Vec<u8> = vec![
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0, // row 0
            12, 34, 56, 200, 99, 88, 77, 255, 250, 240, 230, 17, // row 1
        ];
        let png = encode_png_rgba(width, height, &rgba).expect("encode valid RGBA");
        assert!(png.starts_with(PNG_SIGNATURE));
        let (dw, dh, decoded) = decode_png_rgba(&png).expect("decode valid PNG");
        assert_eq!((dw, dh), (width, height));
        assert_eq!(decoded, rgba);
    }

    #[test]
    fn decode_supports_all_scanline_filters_and_rgb_color_type() {
        let width = 4u32;
        let height = 5u32;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for i in 0..(width * height) {
            rgba.push(((i * 37) % 256) as u8);
            rgba.push(((i * 73 + 11) % 256) as u8);
            rgba.push(((i * 19 + 200) % 256) as u8);
            rgba.push(((i * 53 + 7) % 256) as u8);
        }
        let png = build_filtered_png(width, height, 6, &rgba, &[0, 1, 2, 3, 4]);
        let (dw, dh, decoded) = decode_png_rgba(&png).expect("decode filtered RGBA PNG");
        assert_eq!((dw, dh), (width, height));
        assert_eq!(decoded, rgba);

        // Also verify 8-bit RGB (color_type == 2) expands alpha to 255.
        let rgb = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
        let rgb_png = build_filtered_png(2, 2, 2, &rgb, &[1, 4]);
        let (rw, rh, decoded_rgb) = decode_png_rgba(&rgb_png).expect("decode filtered RGB PNG");
        assert_eq!((rw, rh), (2, 2));
        assert_eq!(
            decoded_rgb,
            vec![
                10, 20, 30, 255, 40, 50, 60, 255, // row 0
                70, 80, 90, 255, 100, 110, 120, 255, // row 1
            ]
        );
    }

    #[test]
    fn rejects_invalid_inputs_and_corrupted_crc() {
        assert!(encode_png_rgba(0, 1, &[]).is_none());
        assert!(encode_png_rgba(1, 0, &[]).is_none());
        assert!(encode_png_rgba(MAX_PNG_DIMENSION + 1, 1, &[]).is_none());
        assert!(encode_png_rgba(2, 2, &[0u8; 4]).is_none());

        assert!(decode_png_rgba(&[]).is_none());
        assert!(decode_png_rgba(b"not a png").is_none());

        let mut png = encode_png_rgba(1, 1, &[1, 2, 3, 4]).unwrap();
        let last = png.len() - 1;
        png[last] ^= 0xff; // corrupt IEND CRC
        assert!(decode_png_rgba(&png).is_none());
    }
}
