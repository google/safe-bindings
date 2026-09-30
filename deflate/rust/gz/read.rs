use bytes::buf::Reader;
use bytes::{Buf, Bytes};
use flate2::read::GzDecoder as Flate2GzDecoder;
use flate2::read::GzEncoder as Flate2GzEncoder;
use flate2::read::MultiGzDecoder as Flate2MultiGzDecoder;
use flate2::GzHeader;
use std::io::Read;

use crate::vec_u8::VecU8;
use crate::Compression;

/// Reads `reader` to the end, pre-allocating `capacity_hint` bytes for the
/// output.
///
/// `Vec` grows geometrically, so without an accurate hint the returned buffer
/// can have up to ~2x more capacity than its length. Since callers typically
/// hand the whole allocation off (e.g. to an `absl::Cord`), that slack would
/// be retained for the lifetime of the result.
fn read_to_end_impl<R: Read>(reader: &mut Option<R>, capacity_hint: usize) -> Result<VecU8, VecU8> {
    if let Some(reader) = reader {
        let mut vec: Vec<u8> = Vec::with_capacity(capacity_hint);
        match reader.read_to_end(&mut vec) {
            Ok(_) => Ok(VecU8::from(vec)),
            Err(e) => Err(VecU8::from(e.to_string())),
        }
    } else {
        Err(VecU8::from("No reader available"))
    }
}

/// Minimal gzip member: 10 byte header, 2 byte empty deflate block and 8 byte
/// trailer.
const MIN_GZIP_SIZE: usize = 20;

/// Upper bound on the deflate compression ratio (see
/// https://zlib.net/zlib_tech.html). Used to bound the capacity we're willing
/// to pre-allocate based on the untrusted ISIZE field.
const MAX_DEFLATE_RATIO: usize = 1032;

/// Returns the uncompressed size stored in the gzip ISIZE trailer (the last 4
/// bytes of the stream, little-endian), to be used as an allocation hint.
///
/// ISIZE is the uncompressed size modulo 2^32 of the *last* member, and the
/// input is untrusted, so this is only ever a hint: it's clamped to what the
/// input could plausibly decompress to, and `read_to_end` still grows the
/// buffer if the hint turns out to be too small.
fn gzip_uncompressed_size_hint(buf: &[u8]) -> usize {
    if buf.len() < MIN_GZIP_SIZE || !buf.starts_with(&[0x1f, 0x8b]) {
        return 0;
    }
    let trailer: [u8; 4] = buf[buf.len() - 4..].try_into().unwrap();
    let uncompressed_size = u32::from_le_bytes(trailer) as usize;
    uncompressed_size.min(buf.len().saturating_mul(MAX_DEFLATE_RATIO))
}

fn header_impl(header: Option<&GzHeader>) -> Option<GzHeader> {
    match header {
        Some(h) => Some(h.clone()),
        None => None.into(),
    }
}

#[derive(Default)]
pub struct GzDecoder {
    reader: Option<Flate2GzDecoder<Reader<Bytes>>>,
    /// Expected uncompressed size, derived from the gzip trailer.
    size_hint: usize,
}

#[derive(Default)]
pub struct GzEncoder {
    reader: Option<Flate2GzEncoder<Reader<Bytes>>>,
}

#[derive(Default)]
pub struct MultiGzDecoder {
    reader: Option<Flate2MultiGzDecoder<Reader<Bytes>>>,
}

impl GzDecoder {
    /// Creates a new GzDecoder. Data is read from the given stream.
    pub fn create(buf: &[u8]) -> Self {
        let size_hint = gzip_uncompressed_size_hint(buf);
        let buf_inner = Bytes::copy_from_slice(buf).reader();
        Self { reader: Some(Flate2GzDecoder::new(buf_inner)), size_hint }
    }

    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> Option<GzHeader> {
        header_impl(self.reader.as_ref().and_then(|r| r.header()))
    }

    /// Reads the entire stream into a Result<VecU8, VecU8>.
    ///
    /// The output buffer is pre-sized using the gzip ISIZE trailer, so for
    /// well-formed single-member input its capacity matches its length.
    pub fn read_to_end(&mut self) -> Result<VecU8, VecU8> {
        read_to_end_impl(&mut self.reader, self.size_hint)
    }
}

impl GzEncoder {
    /// Creates a new GzEncoder. Data is read from the given stream.
    pub fn create(buf: &[u8], level: Compression) -> Self {
        let buf_inner = Bytes::copy_from_slice(buf).reader();
        Self { reader: Some(Flate2GzEncoder::new(buf_inner, level)) }
    }

    /// Reads the entire stream into a Result<VecU8, VecU8>.
    pub fn read_to_end(&mut self) -> Result<VecU8, VecU8> {
        read_to_end_impl(&mut self.reader, 0)
    }
}

impl MultiGzDecoder {
    /// Creates a new MultiGzDecoder. Data is read from the given stream.
    pub fn create(buf: &[u8]) -> Self {
        let buf_inner = Bytes::copy_from_slice(buf).reader();
        Self { reader: Some(Flate2MultiGzDecoder::new(buf_inner)) }
    }

    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> Option<flate2::GzHeader> {
        header_impl(self.reader.as_ref().and_then(|r| r.header()))
    }

    /// Reads the entire stream into a Result<VecU8, VecU8>.
    pub fn read_to_end(&mut self) -> Result<VecU8, VecU8> {
        read_to_end_impl(&mut self.reader, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder as Flate2WriteGzEncoder;
    use googletest::prelude::*;
    use std::io::Write;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = Flate2WriteGzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[gtest]
    fn gz_decoder_output_is_exactly_sized() {
        // Pick a size that isn't a power of two so that geometric growth
        // would leave slack.
        let data: Vec<u8> = (0..100_003u32).map(|i| (i % 251) as u8).collect();
        let compressed = gzip(&data);

        let mut decoder = GzDecoder::create(&compressed);
        let out = decoder.read_to_end().unwrap();
        expect_eq!(out.as_vec(), &data);
        expect_eq!(out.as_vec().capacity(), data.len());
    }

    #[gtest]
    fn gz_decoder_empty_input() {
        let compressed = gzip(b"");
        let mut decoder = GzDecoder::create(&compressed);
        let out = decoder.read_to_end().unwrap();
        expect_true!(out.is_empty());
    }

    #[gtest]
    fn gz_decoder_wrong_isize_does_not_crash() {
        let data = b"hello hello hello hello hello".repeat(100);
        let mut compressed = gzip(&data);
        let n = compressed.len();
        // Too small: buffer must still grow.
        compressed[n - 4..].copy_from_slice(&1u32.to_le_bytes());
        let mut decoder = GzDecoder::create(&compressed);
        // The ISIZE mismatch is reported as a corrupt stream by flate2, but
        // we must not crash or over-allocate getting there.
        expect_true!(decoder.read_to_end().is_err());
    }

    #[gtest]
    fn size_hint_is_clamped() {
        let mut buf = vec![0x1f, 0x8b];
        buf.resize(MIN_GZIP_SIZE, 0);
        let n = buf.len();
        buf[n - 4..].copy_from_slice(&u32::MAX.to_le_bytes());
        expect_eq!(gzip_uncompressed_size_hint(&buf), MIN_GZIP_SIZE * MAX_DEFLATE_RATIO);
    }

    /// Builds a `len`-byte buffer with the given prefix and a non-zero ISIZE
    /// trailer, so that any input that isn't rejected yields a non-zero hint.
    fn buf_with_trailer(prefix: &[u8], len: usize) -> Vec<u8> {
        let mut buf = prefix.to_vec();
        buf.resize(len, 0);
        buf[len - 4..].copy_from_slice(&7u32.to_le_bytes());
        buf
    }

    #[gtest]
    fn size_hint_reads_trailer() {
        let buf = buf_with_trailer(&[0x1f, 0x8b], MIN_GZIP_SIZE);
        expect_eq!(gzip_uncompressed_size_hint(&buf), 7);
    }

    #[gtest]
    fn size_hint_ignores_input_without_gzip_magic() {
        let bad_id1 = buf_with_trailer(&[0x00, 0x8b], 64);
        let bad_id2 = buf_with_trailer(&[0x1f, 0x00], 64);
        expect_eq!(gzip_uncompressed_size_hint(&bad_id1), 0);
        expect_eq!(gzip_uncompressed_size_hint(&bad_id2), 0);
    }

    #[gtest]
    fn size_hint_ignores_input_shorter_than_min_gzip() {
        let too_short = buf_with_trailer(&[0x1f, 0x8b], MIN_GZIP_SIZE - 1);
        expect_eq!(gzip_uncompressed_size_hint(&too_short), 0);
        expect_eq!(gzip_uncompressed_size_hint(b"short"), 0);
        expect_eq!(gzip_uncompressed_size_hint(&[]), 0);
    }
}
