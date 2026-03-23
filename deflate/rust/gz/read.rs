use anyhow;
use bytes::buf::Reader;
use bytes::{Buf, Bytes};
use cc_std::std::string_view;
use flate2::read::GzDecoder as Flate2GzDecoder;
use flate2::read::GzEncoder as Flate2GzEncoder;
use flate2::read::MultiGzDecoder as Flate2MultiGzDecoder;
use flate2::GzHeader;
use rust_vec_u8::VecU8;
use std::io::Read;

use super::OptionGzHeader;
use crate::{Compression, ResultVecU8};

fn read_to_end_impl<R: Read>(reader: &mut Option<R>) -> ResultVecU8 {
    if let Some(reader) = reader {
        let mut vec: Vec<u8> = Vec::new();
        let result = reader.read_to_end(&mut vec);
        match result {
            Ok(_) => Ok(VecU8::from(vec)).into(),
            Err(e) => Err(anyhow::anyhow!(e)).into(),
        }
    } else {
        Err(anyhow::anyhow!("No reader available")).into()
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
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn create(buf: string_view) -> Self {
        // SAFETY: The unsafe operation is dereferencing the raw pointer returned by
        // `buf.as_bytes()` using `&*`. This is necessary because we need to copy the data from
        // the C++ raw_string_view to create a new `Bytes` object. The operation is safe as long
        // as the raw_string_view points to a valid memory region and remains unmodified during the
        // call, which should be guaranteed with the SAFETY contract. We ensure the reference
        // resulting from `buf.as_bytes()` does not escape the current function by deep copying
        // the result using `copy_from_slice`.

        let buf_inner = Bytes::copy_from_slice(unsafe { buf.as_bytes() }).reader();
        Self { reader: Some(Flate2GzDecoder::new(buf_inner)) }
    }

    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> OptionGzHeader {
        header_impl(self.reader.as_ref().and_then(|r| r.header()))
    }

    /// Reads the entire stream into a VecU8.
    pub fn read_to_end(&mut self) -> ResultVecU8 {
        read_to_end_impl(&mut self.reader)
    }
}

impl GzEncoder {
    /// Creates a new GzEncoder. Data is read from the given stream.
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn create(buf: string_view, level: Compression) -> Self {
        // SAFETY: This function has the same requirements as the `create` function of GzDecoder.

        let buf_inner = Bytes::copy_from_slice(unsafe { buf.as_bytes() }).reader();
        Self { reader: Some(Flate2GzEncoder::new(buf_inner, level)) }
    }

    /// Reads the entire stream into a VecU8.
    pub fn read_to_end(&mut self) -> ResultVecU8 {
        read_to_end_impl(&mut self.reader)
    }
}

impl MultiGzDecoder {
    /// Creates a new MultiGzDecoder. Data is read from the given stream.
    ///
    /// # Safety
    ///
    /// `buf` must come from a valid C++ string_view, and its lifetime needs to exceed the
    /// function call. The underlying bytes must not be written to during the function call.
    pub unsafe fn create(buf: string_view) -> Self {
        // SAFETY: This function has the same requirements as the `create` function of GzDecoder.

        let buf_inner = Bytes::copy_from_slice(unsafe { buf.as_bytes() }).reader();
        Self { reader: Some(Flate2MultiGzDecoder::new(buf_inner)) }
    }

    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> OptionGzHeader {
        header_impl(self.reader.as_ref().and_then(|r| r.header()))
    }

    /// Reads the entire stream into a VecU8.
    pub fn read_to_end(&mut self) -> ResultVecU8 {
        read_to_end_impl(&mut self.reader)
    }
}
