use flate2::write::GzDecoder as Flate2GzDecoder;
use flate2::write::GzEncoder as Flate2GzEncoder;
use flate2::write::MultiGzDecoder as Flate2MultiGzDecoder;
use flate2::GzHeader;
use std::io::Write;

use crate::vec_u8::VecU8;
use crate::Compression;

// NOTE: b/517030085 - Crubit doesn't seem to support the unit type here, so using a u8 for now.
fn write_all_impl<W: Write>(writer: &mut Option<W>, bytes: &[u8]) -> Result<u8, VecU8> {
    if let Some(writer) = writer {
        match writer.write_all(bytes) {
            Ok(()) => Ok(0),
            Err(e) => Err(VecU8::from(e.to_string())),
        }
    } else {
        Err(VecU8::from("No writer available"))
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
    writer: Option<Flate2GzDecoder<Vec<u8>>>,
}

#[derive(Default)]
pub struct GzEncoder {
    writer: Option<Flate2GzEncoder<Vec<u8>>>,
}

#[derive(Default)]
pub struct MultiGzDecoder {
    writer: Option<Flate2MultiGzDecoder<Vec<u8>>>,
}

impl GzDecoder {
    pub fn create() -> Self {
        Self { writer: Some(Flate2GzDecoder::new(Vec::new())) }
    }

    pub fn header(&self) -> Option<GzHeader> {
        header_impl(self.writer.as_ref().and_then(|w| w.header()))
    }

    /// Attempts to write an entire buffer into this writer.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<u8, VecU8> {
        write_all_impl(&mut self.writer, buf)
    }

    pub fn finish(&mut self) -> Result<VecU8, VecU8> {
        if let Some(writer) = self.writer.take() {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)),
                Err(e) => Err(VecU8::from(e.to_string())),
            }
        } else {
            Err(VecU8::from("No writer available"))
        }
    }
}

impl GzEncoder {
    pub fn create(level: Compression) -> Self {
        Self { writer: Some(Flate2GzEncoder::new(Vec::new(), level)) }
    }

    /// Attempts to write an entire buffer into this writer.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<u8, VecU8> {
        write_all_impl(&mut self.writer, buf)
    }

    pub fn finish(&mut self) -> Result<VecU8, VecU8> {
        if let Some(writer) = self.writer.take() {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)),
                Err(e) => Err(VecU8::from(e.to_string())),
            }
        } else {
            Err(VecU8::from("No writer available"))
        }
    }
}

impl MultiGzDecoder {
    pub fn create() -> Self {
        Self { writer: Some(Flate2MultiGzDecoder::new(Vec::new())) }
    }

    pub fn header(&self) -> Option<GzHeader> {
        header_impl(self.writer.as_ref().and_then(|w| w.header()))
    }

    /// Attempts to write an entire buffer into this writer.
    pub fn write_all(&mut self, buf: &[u8]) -> Result<u8, VecU8> {
        write_all_impl(&mut self.writer, buf)
    }

    pub fn finish(&mut self) -> Result<VecU8, VecU8> {
        if let Some(writer) = self.writer.take() {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)),
                Err(e) => Err(VecU8::from(e.to_string())),
            }
        } else {
            Err(VecU8::from("No writer available"))
        }
    }
}
