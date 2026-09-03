//! C++ bindings for writing XML via the Rust crate `quick-xml` (`quick_xml::writer`).
//!
//! WARNING: This crate should never be used from Rust, instead use `quick-xml` directly.

use quick_xml::events::{
    BytesCData, BytesDecl, BytesEnd, BytesPI, BytesRef, BytesStart, BytesText, Event,
};
use quick_xml::writer::Writer as PlainWriter;
use std::str;

// LINT.IfChange(WriterConfig)
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuickWriterConfig {
    pub add_space_before_slash_in_empty_elements: bool,
    pub perform_indent: bool,
    pub indent_char: u8,
    pub indent_size: usize,
}

impl Default for QuickWriterConfig {
    fn default() -> Self {
        Self {
            add_space_before_slash_in_empty_elements: false,
            perform_indent: false,
            indent_char: b' ',
            indent_size: 2,
        }
    }
}
// LINT.ThenChange(
//     ../quick_writer.h:WriterConfig,
//     ../quick_writer.cc:WriterConfig
// )

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct WriterError {
    pub message: Vec<u8>,
}

impl WriterError {
    pub fn new(message: impl Into<Vec<u8>>) -> Self {
        Self { message: message.into() }
    }
}

struct PendingStartElement {
    name: String,
    attributes: Vec<(String, String)>,
}

pub struct QuickWriter {
    writer: PlainWriter<Vec<u8>>,
    pending_start: Option<PendingStartElement>,
}

/// A dummy-default implementation that should not be used directly from the Rust side.
/// This exists to make the resulting C++ bindings-type `movable`. See
/// go/crubit/rust/movable_types for more.
impl Default for QuickWriter {
    fn default() -> Self {
        Self::new_with_config(QuickWriterConfig::default())
    }
}

impl QuickWriter {
    pub fn new() -> Self {
        Self::new_with_config(QuickWriterConfig::default())
    }

    pub fn new_with_config(config: QuickWriterConfig) -> Self {
        let mut writer = if config.perform_indent {
            PlainWriter::new_with_indent(Vec::new(), config.indent_char, config.indent_size)
        } else {
            PlainWriter::new(Vec::new())
        };
        writer.config_mut().add_space_before_slash_in_empty_elements =
            config.add_space_before_slash_in_empty_elements;
        Self { writer, pending_start: None }
    }

    fn flush_pending_start(&mut self) -> Result<bool, WriterError> {
        if let Some(pending) = self.pending_start.take() {
            let mut elem = BytesStart::new(pending.name.as_str());
            for (attr_name, attr_val) in &pending.attributes {
                elem.push_attribute((attr_name.as_str(), attr_val.as_str()));
            }
            self.writer
                .write_event(Event::Start(elem))
                .map_err(|e| WriterError::new(e.to_string().as_bytes()))?;
        }
        Ok(true)
    }

    pub fn write_start_document(&mut self) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let decl = BytesDecl::new("1.0", Some("UTF-8"), None);
        self.writer
            .write_event(Event::Decl(decl))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_start_element(&mut self, name: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let name_str = str::from_utf8(name)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in tag name"[..]))?;
        self.pending_start =
            Some(PendingStartElement { name: name_str.to_string(), attributes: Vec::new() });
        Ok(true)
    }

    pub fn write_attribute(&mut self, name: &[u8], value: &[u8]) -> Result<bool, WriterError> {
        let name_str = str::from_utf8(name)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in attribute name"[..]))?;
        let value_str = str::from_utf8(value)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in attribute value"[..]))?;
        if let Some(pending) = self.pending_start.as_mut() {
            pending.attributes.push((name_str.to_string(), value_str.to_string()));
            Ok(true)
        } else {
            Err(WriterError::new(&b"Cannot add attribute when no element is open"[..]))
        }
    }

    pub fn write_end_element(&mut self, name: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let end = if name.is_empty() {
            return Err(WriterError::new(&b"End element name required"[..]));
        } else {
            let name_str = str::from_utf8(name)
                .map_err(|_| WriterError::new(&b"Invalid UTF-8 in tag name"[..]))?;
            BytesEnd::new(name_str)
        };
        self.writer
            .write_event(Event::End(end))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_empty_element(&mut self, name: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let name_str = str::from_utf8(name)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in tag name"[..]))?;
        self.writer
            .write_event(Event::Empty(BytesStart::new(name_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_characters(&mut self, text: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let text_str = str::from_utf8(text)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in characters"[..]))?;
        self.writer
            .write_event(Event::Text(BytesText::new(text_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_cdata(&mut self, text: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let text_str =
            str::from_utf8(text).map_err(|_| WriterError::new(&b"Invalid UTF-8 in CDATA"[..]))?;
        self.writer
            .write_event(Event::CData(BytesCData::new(text_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_comment(&mut self, text: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let text_str =
            str::from_utf8(text).map_err(|_| WriterError::new(&b"Invalid UTF-8 in comment"[..]))?;
        self.writer
            .write_event(Event::Comment(BytesText::from_escaped(text_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_processing_instruction(&mut self, content: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let content_str =
            str::from_utf8(content).map_err(|_| WriterError::new(&b"Invalid UTF-8 in PI"[..]))?;
        self.writer
            .write_event(Event::PI(BytesPI::new(content_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn write_general_reference(&mut self, name: &[u8]) -> Result<bool, WriterError> {
        self.flush_pending_start()?;
        let name_str = str::from_utf8(name)
            .map_err(|_| WriterError::new(&b"Invalid UTF-8 in general reference"[..]))?;
        self.writer
            .write_event(Event::GeneralRef(BytesRef::new(name_str)))
            .map(|_| true)
            .map_err(|e| WriterError::new(e.to_string().as_bytes()))
    }

    pub fn content(&mut self) -> Result<&[u8], WriterError> {
        self.flush_pending_start()?;
        Ok(self.writer.get_ref().as_slice())
    }
}
