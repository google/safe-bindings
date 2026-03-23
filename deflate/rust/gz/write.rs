use anyhow;
use cc_std::std::string_view;
use flate2::write::GzDecoder as Flate2GzDecoder;
use flate2::write::GzEncoder as Flate2GzEncoder;
use flate2::write::MultiGzDecoder as Flate2MultiGzDecoder;
use flate2::GzHeader;
use rust_vec_u8::VecU8;
use std::io::Write;

use super::OptionGzHeader;
use crate::{Compression, ResultUnit, ResultVecU8};

fn write_all_impl<W: Write>(writer: &mut Option<W>, bytes: &[u8]) -> ResultUnit {
    if let Some(writer) = writer {
        match writer.write_all(bytes) {
            Ok(()) => ().into(),
            Err(e) => Err(anyhow::anyhow!(e)).into(),
        }
    } else {
        Err(anyhow::anyhow!("No writer available")).into()
    }
}

fn header_impl(header: Option<&GzHeader>) -> OptionGzHeader {
    match header {
        Some(h) => OptionGzHeader::from(h.clone()),
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

    pub fn header(&self) -> OptionGzHeader {
        header_impl(self.writer.as_ref().and_then(|w| w.header()))
    }

    /// Attempts to write an entire buffer into this writer.
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn write_all(&mut self, buf: string_view) -> ResultUnit {
        // SAFETY: We are getting a reference to the raw bytes of a string_view using the
        // unsafe function `as_bytes`. This function is safe as long as the lifetime of
        // string_view exceeds the lifetime of this function and the string_view remains
        // unmodified for that duration, which should be guaranteed with the SAFETY contract.
        // The reference to the string_view's data is only temporary, it will be dropped
        // after "writer.write_all" inside "write_all_impl". Therefore no reference to the
        // string_view escape this function.
        write_all_impl(&mut self.writer, unsafe { buf.as_bytes() })
    }

    pub fn finish(self) -> ResultVecU8 {
        if let Some(writer) = self.writer {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)).into(),
                Err(e) => Err(anyhow::anyhow!(e)).into(),
            }
        } else {
            Err(anyhow::anyhow!("No writer available")).into()
        }
    }
}

impl GzEncoder {
    pub fn create(level: Compression) -> Self {
        Self { writer: Some(Flate2GzEncoder::new(Vec::new(), level)) }
    }

    /// Attempts to write an entire buffer into this writer.
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn write_all(&mut self, buf: string_view) -> ResultUnit {
        // SAFETY: We are getting a reference to the raw bytes of a string_view using the
        // unsafe function `as_bytes`. This function is safe as long as the lifetime of
        // string_view exceeds the lifetime of this function and the string_view remains
        // unmodified for that duration, which should be guaranteed with the SAFETY contract.
        // The reference to the string_view's data is only temporary, it will be dropped
        // after "writer.write_all" inside "write_all_impl". Therefore no reference to
        // the string_view escape this function.
        write_all_impl(&mut self.writer, unsafe { buf.as_bytes() })
    }

    pub fn finish(self) -> ResultVecU8 {
        if let Some(writer) = self.writer {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)).into(),
                Err(e) => Err(anyhow::anyhow!(e)).into(),
            }
        } else {
            Err(anyhow::anyhow!("No writer available")).into()
        }
    }
}

impl MultiGzDecoder {
    pub fn create() -> Self {
        Self { writer: Some(Flate2MultiGzDecoder::new(Vec::new())) }
    }

    pub fn header(&self) -> OptionGzHeader {
        header_impl(self.writer.as_ref().and_then(|w| w.header()))
    }

    /// Attempts to write an entire buffer into this writer.
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn write_all(&mut self, buf: string_view) -> ResultUnit {
        // SAFETY: We are getting a reference to the raw bytes of a string_view using the
        // unsafe function `as_bytes`. This function is safe as long as the lifetime of
        // string_view exceeds the lifetime of this function and the string_view remains
        // unmodified for that duration, which should be guaranteed with the SAFETY contract.
        // The reference to the string_view's data is only temporary, it will be dropped
        // after "writer.write_all" inside "write_all_impl". Therefore no reference to
        // the string_view escape this function.
        write_all_impl(&mut self.writer, unsafe { buf.as_bytes() })
    }

    pub fn finish(self) -> ResultVecU8 {
        if let Some(writer) = self.writer {
            match writer.finish() {
                Ok(buf) => Ok(VecU8::from(buf)).into(),
                Err(e) => Err(anyhow::anyhow!(e)).into(),
            }
        } else {
            Err(anyhow::anyhow!("No writer available")).into()
        }
    }
}
