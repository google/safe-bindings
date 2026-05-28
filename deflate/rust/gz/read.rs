use bytes::buf::Reader;
use bytes::{Buf, Bytes};
use flate2::read::GzDecoder as Flate2GzDecoder;
use flate2::read::GzEncoder as Flate2GzEncoder;
use flate2::read::MultiGzDecoder as Flate2MultiGzDecoder;
use flate2::GzHeader;
use std::io::Read;

use crate::vec_u8::VecU8;
use crate::Compression;

fn read_to_end_impl<R: Read>(reader: &mut Option<R>) -> Result<VecU8, VecU8> {
    if let Some(reader) = reader {
        let mut vec: Vec<u8> = Vec::new();
        match reader.read_to_end(&mut vec) {
            Ok(_) => Ok(VecU8::from(vec)),
            Err(e) => Err(VecU8::from(e.to_string())),
        }
    } else {
        Err(VecU8::from("No reader available"))
    }
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
        let buf_inner = Bytes::copy_from_slice(buf).reader();
        Self { reader: Some(Flate2GzDecoder::new(buf_inner)) }
    }

    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> Option<GzHeader> {
        header_impl(self.reader.as_ref().and_then(|r| r.header()))
    }

    /// Reads the entire stream into a Result<VecU8, VecU8>.
    pub fn read_to_end(&mut self) -> Result<VecU8, VecU8> {
        read_to_end_impl(&mut self.reader)
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
        read_to_end_impl(&mut self.reader)
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
        read_to_end_impl(&mut self.reader)
    }
}
