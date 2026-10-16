//! Checks that prove nothing was lost in a conversion: byte comparison,
//! pixel comparison and reading the size of a result file.

use std::fs::File;
use std::io::{self, ErrorKind, Read};
use std::path::Path;

use image::{DynamicImage, ImageReader};

/// Bytes compared per step; two buffers of this size are held at a time.
const BLOCK_LEN: usize = 256 * 1024;

/// `true` if both files have exactly the same bytes. Compares the sizes first,
/// then reads both files block by block.
pub fn files_identical(a: &Path, b: &Path) -> io::Result<bool> {
    let (a, b) = (File::open(a)?, File::open(b)?);
    if a.metadata()?.len() != b.metadata()?.len() {
        return Ok(false);
    }
    same_bytes(a, b)
}

fn same_bytes(mut a: impl Read, mut b: impl Read) -> io::Result<bool> {
    let mut block_a = vec![0u8; BLOCK_LEN];
    let mut block_b = vec![0u8; BLOCK_LEN];
    loop {
        let len_a = read_block(&mut a, &mut block_a)?;
        let len_b = read_block(&mut b, &mut block_b)?;
        if len_a != len_b || block_a[..len_a] != block_b[..len_b] {
            return Ok(false);
        }
        if len_a == 0 {
            return Ok(true);
        }
    }
}

/// Reads until `buf` is full or the input ends; returns the bytes read.
/// (A single `read` may return less, and not equally for both files.)
fn read_block(input: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match input.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// `true` if both images have the same size and the same pixel values at
/// 16 bit per channel (RGB8 and RGBA8 with full alpha compare equal).
pub fn same_pixels(a: &DynamicImage, b: &DynamicImage) -> bool {
    a.width() == b.width()
        && a.height() == b.height()
        && a.to_rgba16().as_raw() == b.to_rgba16().as_raw()
}

/// Width and height of an image file, read from its header. The format is
/// recognised by content, so `.tmp` files work. On failure a short German
/// reason, ready to show to the user.
pub fn image_dims(path: &Path) -> Result<(u32, u32), String> {
    let reader = ImageReader::open(path)
        .and_then(ImageReader::with_guessed_format)
        .map_err(|e| format!("Datei lässt sich nicht öffnen: {e}"))?;
    reader
        .into_dimensions()
        .map_err(|e| format!("Bild lässt sich nicht lesen: {e}"))
}

/// `true` if the two sizes are equal or swapped. Photos with an EXIF
/// orientation of 5 to 8 are reported rotated by some readers.
pub fn dims_match(expected: (u32, u32), actual: (u32, u32)) -> bool {
    actual == expected || actual == (expected.1, expected.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage, Rgba, RgbaImage};
    use std::fs;

    /// Reader that hands out at most `chunk` bytes per call.
    struct Chunked<'a> {
        data: &'a [u8],
        chunk: usize,
    }

    impl Read for Chunked<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.chunk.min(buf.len()).min(self.data.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 % 251) as u8).collect()
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn identical_files_are_identical() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = pattern(700_000);
        let a = write(dir.path(), "a.bin", &bytes);
        let b = write(dir.path(), "b.bin", &bytes);
        assert!(files_identical(&a, &b).unwrap());
        assert!(files_identical(&a, &a).unwrap());
    }

    #[test]
    fn empty_files_are_identical() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "a.bin", &[]);
        let b = write(dir.path(), "b.bin", &[]);
        assert!(files_identical(&a, &b).unwrap());
    }

    #[test]
    fn different_length_is_not_identical() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = pattern(1000);
        let a = write(dir.path(), "a.bin", &bytes);
        let b = write(dir.path(), "b.bin", &bytes[..999]);
        assert!(!files_identical(&a, &b).unwrap());
        assert!(!files_identical(&b, &a).unwrap());
    }

    #[test]
    fn same_length_but_different_byte_is_not_identical() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = pattern(700_000);
        let a = write(dir.path(), "a.bin", &bytes);
        for at in [0, 262_143, 262_144, 699_999] {
            let mut other = bytes.clone();
            other[at] ^= 1;
            let b = write(dir.path(), "b.bin", &other);
            assert!(!files_identical(&a, &b).unwrap(), "differs at {at}");
        }
    }

    #[test]
    fn missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(dir.path(), "a.bin", &[1]);
        let err = files_identical(&a, &dir.path().join("none.bin")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn comparison_survives_short_reads() {
        let bytes = pattern(300_000);
        // The readers return different, small amounts per call.
        let a = Chunked {
            data: &bytes,
            chunk: 7,
        };
        let b = Chunked {
            data: &bytes,
            chunk: 40_000,
        };
        assert!(same_bytes(a, b).unwrap());

        let mut other = bytes.clone();
        *other.last_mut().unwrap() ^= 1;
        let a = Chunked {
            data: &bytes,
            chunk: 7,
        };
        let b = Chunked {
            data: &other,
            chunk: 40_000,
        };
        assert!(!same_bytes(a, b).unwrap());

        let a = Chunked {
            data: &bytes,
            chunk: 7,
        };
        let b = Chunked {
            data: &bytes[..bytes.len() - 1],
            chunk: 4096,
        };
        assert!(!same_bytes(a, b).unwrap());
    }

    fn rgb(w: u32, h: u32) -> RgbImage {
        RgbImage::from_fn(w, h, |x, y| Rgb([x as u8 * 7, y as u8 * 9, (x + y) as u8]))
    }

    fn with_full_alpha(img: &RgbImage) -> RgbaImage {
        RgbaImage::from_fn(img.width(), img.height(), |x, y| {
            let p = img.get_pixel(x, y);
            Rgba([p[0], p[1], p[2], 255])
        })
    }

    #[test]
    fn rgb8_equals_rgba8_with_full_alpha() {
        let plain = rgb(8, 6);
        let a = DynamicImage::ImageRgb8(plain.clone());
        let b = DynamicImage::ImageRgba8(with_full_alpha(&plain));
        assert!(same_pixels(&a, &b));
        assert!(same_pixels(&b, &a));
    }

    #[test]
    fn one_different_pixel_is_found() {
        let plain = rgb(8, 6);
        let a = DynamicImage::ImageRgb8(plain.clone());
        let mut changed = with_full_alpha(&plain);
        let mut p = *changed.get_pixel(7, 5);
        p[2] ^= 1;
        changed.put_pixel(7, 5, p);
        assert!(!same_pixels(&a, &DynamicImage::ImageRgba8(changed)));
    }

    #[test]
    fn different_alpha_is_found() {
        let plain = rgb(4, 4);
        let a = DynamicImage::ImageRgb8(plain.clone());
        let mut translucent = with_full_alpha(&plain);
        translucent.get_pixel_mut(0, 0)[3] = 254;
        assert!(!same_pixels(&a, &DynamicImage::ImageRgba8(translucent)));
    }

    #[test]
    fn different_size_is_not_the_same() {
        let a = DynamicImage::ImageRgb8(rgb(8, 6));
        let b = DynamicImage::ImageRgb8(rgb(6, 8));
        let c = DynamicImage::ImageRgb8(rgb(8, 7));
        assert!(!same_pixels(&a, &b));
        assert!(!same_pixels(&a, &c));
    }

    #[test]
    fn sixteen_bit_differences_are_found() {
        let plain = rgb(4, 4);
        let wide = DynamicImage::ImageRgb8(plain.clone()).to_rgb16();
        // Widening 8 to 16 bit keeps the values equal ...
        assert!(same_pixels(
            &DynamicImage::ImageRgb8(plain),
            &DynamicImage::ImageRgb16(wide.clone())
        ));
        // ... but a change in the low byte is a real difference.
        let mut changed = wide.clone();
        changed.get_pixel_mut(2, 2)[0] ^= 1;
        assert!(!same_pixels(
            &DynamicImage::ImageRgb16(wide),
            &DynamicImage::ImageRgb16(changed)
        ));
    }

    #[test]
    fn dims_are_read_from_the_header() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("a.png");
        rgb(13, 7).save(&png).unwrap();
        assert_eq!(image_dims(&png), Ok((13, 7)));

        // The format comes from the content, not the extension.
        let tmp = dir.path().join("a.png.tmp");
        fs::copy(&png, &tmp).unwrap();
        assert_eq!(image_dims(&tmp), Ok((13, 7)));

        let jpg = dir.path().join("a.jpg");
        rgb(20, 11).save(&jpg).unwrap();
        assert_eq!(image_dims(&jpg), Ok((20, 11)));
    }

    #[test]
    fn dims_errors_are_short_german_texts() {
        let dir = tempfile::tempdir().unwrap();
        let err = image_dims(&dir.path().join("fehlt.png")).unwrap_err();
        assert!(err.starts_with("Datei lässt sich nicht öffnen"), "{err}");

        let garbage = write(dir.path(), "kaputt.png", b"this is not an image at all");
        let err = image_dims(&garbage).unwrap_err();
        assert!(err.starts_with("Bild lässt sich nicht lesen"), "{err}");

        let empty = write(dir.path(), "leer.png", &[]);
        let err = image_dims(&empty).unwrap_err();
        assert!(err.starts_with("Bild lässt sich nicht lesen"), "{err}");
    }

    #[test]
    fn equal_dims_match() {
        assert!(dims_match((4000, 3000), (4000, 3000)));
    }

    #[test]
    fn swapped_dims_match() {
        assert!(dims_match((4000, 3000), (3000, 4000)));
        assert!(dims_match((3000, 4000), (4000, 3000)));
    }

    #[test]
    fn different_dims_do_not_match() {
        assert!(!dims_match((4000, 3000), (4000, 3001)));
        assert!(!dims_match((4000, 3000), (3000, 4001)));
        assert!(!dims_match((4000, 3000), (2000, 1500)));
    }

    #[test]
    fn zero_dims_match_themselves() {
        assert!(dims_match((0, 0), (0, 0)));
    }
}
