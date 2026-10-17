//! Reads the box structure of a JXL container to find out whether the file
//! carries JPEG reconstruction data (a `jbrd` box), and the image header.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use jxl_oxide::{InitializeResult, JxlImage};

/// First two bytes of a JXL file without a container.
const CODESTREAM_SIGNATURE: [u8; 2] = [0xFF, 0x0A];
/// Box size field value: the 64-bit size follows the type.
const SIZE_IS_64_BIT: u32 = 1;
/// Box size field value: the box runs to the end of the file.
const SIZE_TO_END: u32 = 0;
/// Size and type of a box.
const HEADER_LEN: u64 = 8;
/// Size, type and 64-bit size of a box.
const LARGE_HEADER_LEN: u64 = 16;

/// `true` if the JXL file at `path` contains a `jbrd` box, i.e. it was made by
/// lossless JPEG transcoding and can give back the original JPEG byte for byte.
///
/// A bare codestream (`FF 0A`) has no boxes and gives `false`, and so does a
/// file that ends in the middle of a box. A box that cannot be valid (size
/// smaller than its own header) is an `InvalidData` error.
pub fn has_jpeg_reconstruction(path: &Path) -> io::Result<bool> {
    contains_jbrd(&mut File::open(path)?)
}

/// Reads just enough of a JXL file to parse its image header (size,
/// orientation, animation); nothing is decoded. `None` when the file cannot
/// be read or the header is missing, broken or truncated.
pub(crate) fn read_header(path: &Path) -> Option<JxlImage> {
    let mut file = File::open(path).ok()?;
    let mut uninit = JxlImage::builder().build_uninit();
    let mut buf = vec![0u8; 4096];
    let mut valid = 0;
    loop {
        let count = file.read(&mut buf[valid..]).ok()?;
        if count == 0 {
            return None;
        }
        valid += count;
        let consumed = uninit.feed_bytes(&buf[..valid]).ok()?;
        buf.copy_within(consumed..valid, 0);
        valid -= consumed;
        match uninit.try_init().ok()? {
            InitializeResult::Initialized(image) => return Some(image),
            InitializeResult::NeedMoreData(more) => uninit = more,
        }
    }
}

/// Walks the top-level boxes, seeking over their contents.
fn contains_jbrd<R: Read + Seek>(file: &mut R) -> io::Result<bool> {
    let end = file.seek(SeekFrom::End(0))?;
    let mut pos = 0;
    // Every pass moves `pos` forward by at least a header, so this ends.
    while end.saturating_sub(pos) >= HEADER_LEN {
        file.seek(SeekFrom::Start(pos))?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header)?;
        if pos == 0 && header[..2] == CODESTREAM_SIGNATURE {
            return Ok(false);
        }
        let size32 = u32::from_be_bytes(header[..4].try_into().unwrap());
        let kind = &header[4..];

        let (size, header_len) = match size32 {
            SIZE_TO_END => (end - pos, HEADER_LEN),
            SIZE_IS_64_BIT => {
                if end - pos < LARGE_HEADER_LEN {
                    return Ok(false);
                }
                let mut size64 = [0u8; 8];
                file.read_exact(&mut size64)?;
                (u64::from_be_bytes(size64), LARGE_HEADER_LEN)
            }
            n => (u64::from(n), HEADER_LEN),
        };
        if size < header_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Ungültige Box-Größe in der JXL-Datei",
            ));
        }
        if kind == b"jbrd" {
            return Ok(true);
        }
        match pos.checked_add(size) {
            Some(next) => pos = next,
            None => return Ok(false),
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// The `JXL ` signature box every container starts with.
    const SIGNATURE: [u8; 12] = [0, 0, 0, 12, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A];

    /// A box with a 32-bit size field.
    fn bmff_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32 + 8).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    /// A box that announces its size in the 64-bit extension field.
    fn large_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = 1u32.to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(&(payload.len() as u64 + 16).to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn container(boxes: &[Vec<u8>]) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        for b in boxes {
            out.extend_from_slice(b);
        }
        out
    }

    fn check(bytes: &[u8]) -> io::Result<bool> {
        contains_jbrd(&mut Cursor::new(bytes))
    }

    fn ftyp() -> Vec<u8> {
        bmff_box(b"ftyp", b"jxl \0\0\0\0jxl ")
    }

    #[test]
    fn container_with_jbrd_is_detected() {
        let bytes = container(&[
            ftyp(),
            bmff_box(b"jbrd", &[1, 2, 3]),
            bmff_box(b"jxlc", &[0xFF, 0x0A, 0, 0]),
        ]);
        assert!(check(&bytes).unwrap());
    }

    #[test]
    fn container_without_jbrd_is_not() {
        let bytes = container(&[
            ftyp(),
            bmff_box(b"Exif", &[0; 20]),
            bmff_box(b"jxlc", &[0xFF, 0x0A, 0, 0]),
        ]);
        assert!(!check(&bytes).unwrap());
    }

    #[test]
    fn jbrd_text_inside_another_box_does_not_count() {
        let bytes = container(&[
            ftyp(),
            bmff_box(b"Exif", b"xxjbrdxx"),
            bmff_box(b"jxlc", b"\0\0\0\x08jbrd"),
        ]);
        assert!(!check(&bytes).unwrap());
    }

    #[test]
    fn bare_codestream_is_not() {
        let mut bytes = vec![0xFF, 0x0A];
        bytes.extend_from_slice(b"\0\0\0\x08jbrd and more bytes of image data");
        assert!(!check(&bytes).unwrap());
    }

    #[test]
    fn box_with_64_bit_size_is_skipped() {
        let bytes = container(&[
            ftyp(),
            large_box(b"free", &[7; 10]),
            bmff_box(b"jbrd", &[1]),
        ]);
        assert!(check(&bytes).unwrap());
    }

    #[test]
    fn payload_of_a_64_bit_box_is_not_scanned() {
        // A misread header would land on the fake box in the payload.
        let bytes = container(&[
            ftyp(),
            large_box(b"free", b"\0\0\0\x08jbrd"),
            bmff_box(b"jxlc", &[0xFF, 0x0A]),
        ]);
        assert!(!check(&bytes).unwrap());
    }

    #[test]
    fn jbrd_with_64_bit_size_is_detected() {
        let bytes = container(&[ftyp(), large_box(b"jbrd", &[1, 2, 3])]);
        assert!(check(&bytes).unwrap());
    }

    #[test]
    fn size_zero_means_up_to_the_end_of_the_file() {
        let mut open_ended_jxlc = 0u32.to_be_bytes().to_vec();
        open_ended_jxlc.extend_from_slice(b"jxlc");
        open_ended_jxlc.extend_from_slice(b"\0\0\0\x08jbrd");
        let bytes = container(&[ftyp(), open_ended_jxlc]);
        assert!(!check(&bytes).unwrap(), "the payload is not a box");

        let mut open_ended_jbrd = 0u32.to_be_bytes().to_vec();
        open_ended_jbrd.extend_from_slice(b"jbrd");
        open_ended_jbrd.extend_from_slice(&[9; 30]);
        let bytes = container(&[ftyp(), open_ended_jbrd]);
        assert!(check(&bytes).unwrap());
    }

    #[test]
    fn box_smaller_than_its_header_is_an_error() {
        for size in [2u32, 4, 7] {
            let mut bytes = container(&[ftyp()]);
            bytes.extend_from_slice(&size.to_be_bytes());
            bytes.extend_from_slice(b"free");
            bytes.extend_from_slice(&[0; 32]);
            let err = check(&bytes).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "size {size}");
        }
        // 64-bit form: the length must cover the 16 header bytes.
        let mut bytes = container(&[ftyp()]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(b"free");
        bytes.extend_from_slice(&8u64.to_be_bytes());
        bytes.extend_from_slice(&[0; 32]);
        assert_eq!(
            check(&bytes).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn truncated_files_are_not_reconstructible() {
        assert!(!check(&[]).unwrap());
        assert!(!check(&[0, 0, 0]).unwrap());
        assert!(!check(&SIGNATURE[..10]).unwrap());

        // Box announces more bytes than the file has.
        let mut cut = container(&[ftyp(), bmff_box(b"jxlc", &[0; 40])]);
        cut.truncate(cut.len() - 10);
        assert!(!check(&cut).unwrap());

        // The 64-bit length itself is cut off, or absurdly large.
        let mut bytes = container(&[ftyp()]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(b"free");
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        assert!(!check(&bytes).unwrap());
        bytes.truncate(bytes.len() - 4);
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(!check(&bytes).unwrap());
    }

    #[test]
    fn garbage_never_panics_or_hangs() {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for round in 0..500 {
            let len = 4 + (next() % 200) as usize;
            let mut bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            if round % 2 == 0 {
                // A plausible first box size, so the walk gets past the first box.
                bytes[..4].copy_from_slice(&(8 + (next() % 24) as u32).to_be_bytes());
            }
            let _ = check(&bytes);
        }
    }

    #[test]
    fn works_on_files_too() {
        let dir = tempfile::tempdir().unwrap();
        let with = dir.path().join("with.jxl");
        let without = dir.path().join("without.jxl");
        std::fs::write(&with, container(&[ftyp(), bmff_box(b"jbrd", &[1])])).unwrap();
        std::fs::write(&without, container(&[ftyp()])).unwrap();

        assert!(has_jpeg_reconstruction(&with).unwrap());
        assert!(!has_jpeg_reconstruction(&without).unwrap());
        let err = has_jpeg_reconstruction(&dir.path().join("missing.jxl")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
