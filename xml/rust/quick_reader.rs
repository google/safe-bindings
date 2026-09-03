//! This crate provides C++ bindings for the XML parsing part of the Rust crate "quick-xml".
//! This crate defines types and methods that expose the quick-xml API
//! in a way that go/Crubit understands and is able to generate valid C++ headers from.
//!
//! WARNING: This crate should never be used from Rust, instead use quick-xml directly.

use quick_xml::{
    events::{attributes::Attributes as QuickAttributes, Event},
    name::{QName, ResolveResult},
    reader::{NsReader, Reader as PlainReader},
    Error as QuickXmlError,
};
use std::{io, ops::Deref, str};
use xmlparser::XmlCharExt;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XmlErrorCode {
    InvalidArgument,
    FailedPrecondition,
    Internal,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub struct XmlError {
    pub message: Vec<u8>,
    pub code: XmlErrorCode,
}

impl XmlError {
    pub fn new(message: impl Into<Vec<u8>>, code: XmlErrorCode) -> Self {
        Self { message: message.into(), code }
    }

    pub fn invalid_argument(message: impl Into<Vec<u8>>) -> Self {
        Self::new(message, XmlErrorCode::InvalidArgument)
    }

    pub fn failed_precondition(message: impl Into<Vec<u8>>) -> Self {
        Self::new(message, XmlErrorCode::FailedPrecondition)
    }

    pub fn internal(message: impl Into<Vec<u8>>) -> Self {
        Self::new(message, XmlErrorCode::Internal)
    }
}

/// Limit the length of tag names to prevent excessive resource consumption or potential DoS
/// attacks.
fn check_name_len(input: &[u8]) -> anyhow::Result<()> {
    // Match libxml2's XML_MAX_NAME_LENGTH limit.
    const XML_MAX_NAME_LENGTH: usize = 50000;

    if input.len() > XML_MAX_NAME_LENGTH {
        return Err(anyhow::anyhow!(
            "Invalid tag name length: {} exceeds the limit of {}",
            input.len(),
            XML_MAX_NAME_LENGTH
        ));
    }
    Ok(())
}

/// Checks if a given slice is a valid XML NCName in UTF-8.
/// https://www.w3.org/TR/xml-names/#ns-qualnames describes NCNames and their usage. Both a
/// namespace prefix and the local name must be valid NCNames.
/// https://www.w3.org/TR/xml-names/#ns-decl explains that NCNames are valid Names that don't
/// contain a colon. A valid Name is a non-empty string where the first character satisfies
/// `is_xml_name_start_char` and all subsequent characters satisfy `is_xml_name_char`.
fn is_valid_ncname(input: &[u8]) -> bool {
    let Ok(s) = str::from_utf8(input) else {
        return false;
    };
    let mut chars = s.chars();

    let Some(first_char) = chars.next() else {
        return false;
    };
    if !first_char.is_xml_name_start() {
        return false;
    }
    for c in chars {
        if !c.is_xml_name() {
            return false;
        }
    }

    !s.contains(':')
}

/// Normalizes the line endings in the given byte slice.
///
/// XML parsers are expected to turn all line-breaking sequences CRLF or CR into LF.
/// Quick-xml does not do this by default, to be able to keep referring to the input instead of
/// copying+modifying the data.
///
/// But we need to have this functionality and do a copy anyway, so we provide this function.
fn normalize_line_endings(mut input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix(b"\r") {
            // Consume CR or even CRLF and replace with LF.
            input = rest.strip_prefix(b"\n").unwrap_or(rest);
            output.push(b'\n');
        } else {
            // Identity on the rest.
            let scan_len = input.iter().position(|&b| b == b'\r').unwrap_or(input.len());
            output.extend_from_slice(&input[..scan_len]);
            input = &input[scan_len..];
        }
    }
    output
}

/// Matches the whitespace normalization for attribute values done by libxml2.
///
/// It turns all whitespace into spaces. Interestingly, libxml2 does not seem to collapse those
/// into a single space, so it's not done here either.
fn normalize_whitespace(input: &[u8]) -> Vec<u8> {
    let mut bytes = normalize_line_endings(input);
    for byte in bytes.iter_mut() {
        if byte.is_ascii_whitespace() {
            *byte = b' ';
        }
    }
    bytes
}

/// Computes (local name is valid, prefix is valid) for a given `QName`.
fn compute_validity(name: &QName) -> (bool, bool) {
    (
        is_valid_ncname(name.local_name().into_inner()),
        // A None prefix is considered valid.
        name.prefix().is_none_or(|pref| is_valid_ncname(pref.into_inner())),
    )
}

#[derive(Debug, Default, PartialEq)]
pub struct Attribute {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    // Captures whether the namespace and local name are valid XML Names.
    pub local_name_valid: bool,
    // Note: An absent prefix is considered valid, but an empty one is invalid.
    pub prefix_valid: bool,
}

impl Attribute {
    /// Returns the local name portion of the attribute's key.
    pub fn local_name(&self) -> Vec<u8> {
        let qname = QName(self.key.as_slice());
        qname.local_name().into_inner().into()
    }

    /// Returns the namespace prefix of the attribute's key.
    pub fn namespace_prefix(&self) -> Option<Vec<u8>> {
        let qname = QName(self.key.as_slice());
        qname.prefix().map(|prefix| prefix.into_inner().into())
    }
}

fn expand(entity: &[u8]) -> Vec<u8> {
    if entity.len() < 2 {
        return Vec::new();
    }
    if entity[0] == b'#' {
        // See https://www.w3.org/TR/REC-xml/#sec-references for valid characters in numeric
        // entities.
        let (radix, number_bytes, valid_chars_check): (u32, &[u8], fn(&u8) -> bool) =
            if entity[1] == b'x' {
                (16, &entity[2..], |b| b.is_ascii_hexdigit())
            } else {
                (10, &entity[1..], |b| b.is_ascii_digit())
            };
        if !number_bytes.iter().all(valid_chars_check) {
            return Vec::new();
        }
        // Safe to unwrap, because above we checked that all chars are ASCII and hence valid UTF-8.
        let number_str = std::str::from_utf8(number_bytes).unwrap();
        let Ok(codepoint) = u32::from_str_radix(number_str, radix) else {
            return Vec::new();
        };
        let Some(c) = std::char::from_u32(codepoint) else {
            return Vec::new();
        };
        let mut buf = [0u8; 4];
        c.encode_utf8(&mut buf).as_bytes().to_vec()
    } else {
        match entity {
            b"amp" => b"&".to_vec(),
            b"lt" => b"<".to_vec(),
            b"gt" => b">".to_vec(),
            b"quot" => b"\"".to_vec(),
            b"apos" => b"'".to_vec(),
            _ => Vec::new(),
        }
    }
}

// Replaces all entity references in `attribute_value` using `expand` (above). If finding entities
// fails, returns the original attribute value. If the attribute value contains a '<' (signalling an
// invalid attribute value), returns an error.
fn expand_entities_in_attribute_value(attribute_value: &[u8]) -> anyhow::Result<Vec<u8>> {
    // Check for early exit to avoid running a nested parser. If there is no `&` in the attribute
    // value, then there are no entities to expand.
    if !attribute_value.contains(&b'&') {
        return Ok(attribute_value.to_vec());
    }
    // Attribute values must not contain '<' (see https://www.w3.org/TR/xml/#NT-AttValue), so
    // parsing them as XML yields a sequence of Text and GeneralRef events only. This is exactly
    // what is needed in this function: Text are passed through, entities are expanded.
    if attribute_value.contains(&b'<') {
        return Err(anyhow::anyhow!("Invalid attribute value: contains '<'"));
    }
    let mut result = Vec::new();
    let mut reader = PlainReader::from_reader(attribute_value);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(event) => match event {
                Event::Text(ref bytes_text) => {
                    result.extend_from_slice(bytes_text.deref());
                }
                Event::GeneralRef(ref bytes_ref) => {
                    let entity_name = bytes_ref.deref();
                    let expanded = expand(entity_name);
                    if expanded.is_empty() {
                        result.push(b'&');
                        result.extend_from_slice(entity_name);
                        result.push(b';');
                    } else {
                        result.extend_from_slice(&expanded);
                    }
                }
                Event::Eof => break,
                _ => {}
            },
            Err(_) => return Ok(attribute_value.to_vec()),
        }
        buf.clear();
    }
    Ok(result)
}

/// [Attributes] wraps `QuickAttributes` to allow bridging into C++.
/// See https://docs.rs/quick-xml/latest/quick_xml/events/attributes/struct.Attributes.html for the
/// wrapped type.
#[derive(Debug, Default)]
pub struct Attributes<'a> {
    inner: Option<QuickAttributes<'a>>,
    resolving_namespaces: bool,
    yield_xmlns_attributes: bool,
}

impl<'a> Attributes<'a> {
    /// Forwards `Iterator::next` so that Crubit can bridge it into C++.
    pub fn next_attribute(&mut self) -> Result<Option<Attribute>, Vec<u8>> {
        <Self as Iterator>::next(self).transpose()
    }
}

impl<'a> Iterator for Attributes<'a> {
    type Item = Result<Attribute, Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(inner) = &mut self.inner else {
            return None;
        };
        loop {
            let attr_result = inner.next()?;
            match attr_result {
                Ok(attr) => {
                    let (local_name_valid, prefix_valid) = compute_validity(&attr.key);
                    if local_name_valid && self.resolving_namespaces && !self.yield_xmlns_attributes
                    {
                        let (local, prefix) = attr.key.decompose();
                        if match prefix {
                            Some(p) => p.as_ref() == b"xmlns",
                            None => local.as_ref() == b"xmlns",
                        } {
                            // `xmlns` are not usual attributes, and the parser already
                            // transformed that information into namespace resolution. Let's
                            // match what libxml2 was doing and not return these attributes.
                            continue;
                        }
                    }
                    match expand_entities_in_attribute_value(attr.value.as_ref()) {
                        Ok(ref expanded) => {
                            return Some(Ok(Attribute {
                                key: attr.key.into_inner().into(),
                                // https://www.w3.org/TR/xml/#AVNormalize says to expand entities before normalizing whitespace.
                                value: normalize_whitespace(expanded),
                                local_name_valid,
                                prefix_valid,
                            }));
                        }
                        Err(err) => return Some(Err(err.to_string().into())),
                    };
                }
                Err(err) => return Some(Err(err.to_string().into())),
            }
        }
    }
}

/// [EventType] strips the payloads from `Event` to allow bridging into C++.
/// Since `repr(C)` enums require a default value, we have chosen EoF as a safe fallback.
/// NOTE: b/262737383 - Change once Rust Enum is supported by Crubit.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub enum EventType {
    Start,
    End,
    Empty,
    Text,
    CData,
    Comment,
    Decl,
    PI,
    DocType,
    GeneralRef,
    #[default]
    Eof,
}

/// [XmlEvent] wraps a `Event` to allow bridging into C++. See
/// https://docs.rs/quick-xml/latest/quick_xml/events/enum.Event.html for the wrapped type.
/// NOTE: b/262737383 - Change once Rust Enum is supported by Crubit.
#[derive(Debug)]
pub struct XmlEvent<'a> {
    event: Event<'a>,
    resolving_namespaces: bool,
    yield_xmlns_attributes: bool,
    // Captures whether the namespace and local name are valid XML Names. These are only set for
    // Start, Empty and End events.
    pub local_name_valid: bool,
    // Note: An absent prefix is considered valid, but an empty one is invalid.
    pub prefix_valid: bool,
    pub expanded_entity: Vec<u8>, // Only set for GeneralRef events.
}

/// A dummy-default implementation that should not be used from the Rust side.
/// This only exists to make the resulting C++ bindings-type `movable`. See
/// go/crubit/rust/movable_types for more.
impl<'a> Default for XmlEvent<'a> {
    fn default() -> Self {
        Self {
            event: Event::Eof,
            resolving_namespaces: false,
            yield_xmlns_attributes: false,
            local_name_valid: false,
            prefix_valid: false,
            expanded_entity: Vec::new(),
        }
    }
}

// A `ResultXmlEvent` that represents a Crubit-understandable `Result<XmlEvent>`.
// NOTE: b/367916605 - Change once Result is supported by Crubit.

impl<'a> XmlEvent<'a> {
    pub fn event_type(self: &XmlEvent<'a>) -> EventType {
        match &self.event {
            Event::Start(_) => EventType::Start,
            Event::End(_) => EventType::End,
            Event::Empty(_) => EventType::Empty,
            Event::Text(_) => EventType::Text,
            Event::CData(_) => EventType::CData,
            Event::Comment(_) => EventType::Comment,
            Event::Decl(_) => EventType::Decl,
            Event::PI(_) => EventType::PI,
            Event::DocType(_) => EventType::DocType,
            Event::GeneralRef(_) => EventType::GeneralRef,
            Event::Eof => EventType::Eof,
        }
    }

    /// For `Start`, `Empty` and `End` events, returns the tag name without namespace prefix. For
    /// other events, returns an empty string. The copy is necessary, because the name is decoded
    /// and thus potentially no longer a substring of the input.
    pub fn local_name(self: &'a XmlEvent<'a>) -> Vec<u8> {
        match &self.event {
            Event::Start(ref bytes_start) => bytes_start.local_name().as_ref().into(),
            Event::Empty(ref bytes_start) => bytes_start.local_name().as_ref().into(),
            Event::End(ref bytes_end) => bytes_end.local_name().as_ref().into(),
            _ => b"".as_slice().into(),
        }
    }

    /// For `Start`, `Empty` and `End` events, returns the qualified tag name.
    pub fn qname(self: &'a XmlEvent<'a>) -> Vec<u8> {
        match &self.event {
            Event::Start(ref bytes_start) => bytes_start.name().0.into(),
            Event::Empty(ref bytes_start) => bytes_start.name().0.into(),
            Event::End(ref bytes_end) => bytes_end.name().0.into(),
            _ => b"".as_slice().into(),
        }
    }

    /// For `Start`, `Empty` and `End` events, returns the tag name's namespace prefix, if any.
    /// Otherwise returns an empty string.
    pub fn namespace_prefix(self: &'a XmlEvent<'a>) -> Option<Vec<u8>> {
        match &self.event {
            Event::Start(ref bytes_start) => {
                bytes_start.name().prefix().map(|prefix| prefix.into_inner().into())
            }
            Event::Empty(ref bytes_start) => {
                bytes_start.name().prefix().map(|prefix| prefix.into_inner().into())
            }
            Event::End(ref bytes_end) => {
                bytes_end.name().prefix().map(|prefix| prefix.into_inner().into())
            }
            _ => None,
        }
    }

    /// For `Text`, `Comment`, `DocType` and `CData` events, returns the content. For other events,
    /// returns an empty string.
    pub fn content(self: &XmlEvent<'a>) -> Vec<u8> {
        let content: &[u8] = match &self.event {
            Event::Text(ref bytes_text) => bytes_text.deref(),
            Event::Comment(ref bytes_text) => bytes_text.deref(),
            Event::DocType(ref bytes_text) => bytes_text.deref(),
            Event::CData(ref bytes_cdata) => bytes_cdata.deref(),
            Event::Decl(ref bytes_decl) => bytes_decl.deref(),
            Event::PI(ref bytes_pi) => bytes_pi.deref(),
            Event::GeneralRef(ref bytes_ref) => bytes_ref.deref(),
            _ => "".as_bytes(),
        };
        normalize_line_endings(content)
    }

    /// For `Start` and `Empty` events, returns the attributes.
    pub fn attrs(self: &'a mut XmlEvent<'a>) -> Attributes<'a> {
        match &self.event {
            Event::Start(ref bytes_start) | Event::Empty(ref bytes_start) => Attributes {
                inner: Some(bytes_start.attributes().clone()),
                resolving_namespaces: self.resolving_namespaces,
                yield_xmlns_attributes: self.yield_xmlns_attributes,
            },
            _ => Attributes {
                inner: None,
                resolving_namespaces: false,
                yield_xmlns_attributes: false,
            },
        }
    }
}

/// Configuration for the `QuickReader`. The default settings provide the full feature set, but can
/// be switched off to improve performance.
// LINT.IfChange(ReaderConfig)
#[derive(Debug, Clone, Copy)]
pub struct ReaderConfig {
    /// If true, the `QuickReader` will be able to provide substrings of the XML string for each
    /// start-tag event. Default = true.
    pub xml_substrings: bool,
    /// If true, the `QuickReader` will ignore namespaces in the XML string (see the difference
    /// between `Reader` and `NsReader` in the `quick_xml` crate). Default = false.
    pub ignore_namespaces: bool,
    // If true, the `QuickReader` will expand self-closing tags to emit a start
    // and a end event. Default = false.
    pub expand_empty_elements: bool,
    /// If true, the `QuickReader` will always yield `xmlns` declarations as attributes. This
    /// happens anyway if `ignore_namespaces` is `true`. Default = false.
    pub yield_xmlns_attributes: bool,
}
// LINT.ThenChange(
//     ../quick_reader.h:ReaderConfig,
//     ../quick_reader.cc:ReaderConfig
// )

impl Default for ReaderConfig {
    fn default() -> Self {
        Self {
            xml_substrings: true,
            ignore_namespaces: false,
            expand_empty_elements: false,
            yield_xmlns_attributes: false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Range {
    pub outer_begin: usize,
    pub inner_begin: usize,
    pub inner_end: usize,
    pub outer_end: usize,
}

// A `ResultRange` that represents a Crubit-understandable `Result<Range>`.
// NOTE: b/367916605 - Change once Result is supported by Crubit.

#[derive(Debug, Default, PartialEq)]
pub struct ResolvedName {
    pub namespace_uri: Vec<u8>,
    pub local_name: Vec<u8>,
}

enum Reader<'a> {
    Plain(PlainReader<&'a [u8]>),
    Ns(NsReader<&'a [u8]>),
}

impl<'a> Reader<'a> {
    fn read_event(&mut self) -> quick_xml::errors::Result<Event<'a>> {
        match self {
            Reader::Plain(reader) => reader.read_event(),
            Reader::Ns(reader) => reader.read_event(),
        }
    }
}

/// A Crubit-understandable wrapper around the two readers from
/// https://docs.rs/quick-xml/latest/quick_xml/reader/index.html.
/// NOTE: b/259749095 - Use the `quick_xml` type directly, once generics is supported by Crubit.
pub struct QuickReader<'a> {
    reader: Reader<'a>,
    ranges: Option<anyhow::Result<Vec<Range>>>, // empty if the XML substrings feature is disabled
    start_event_count: usize,                   /* how many start and empty events have been
                                                 * parsed so far */
    yield_xmlns_attributes: bool,
}

/// A dummy-default implementation that should not be used from the Rust side.
/// This only exists to make the resulting C++ bindings-type `movable`. See
/// go/crubit/rust/movable_types for more.
impl<'a> Default for QuickReader<'a> {
    fn default() -> Self {
        Self {
            reader: Reader::Plain(quick_xml::reader::Reader::<&'a [u8]>::from_str("")),
            ranges: None,
            start_event_count: 0,
            yield_xmlns_attributes: false,
        }
    }
}

impl<'a> QuickReader<'a> {
    /// Parses the input and computes the ranges of all the start tags.
    fn collect_ranges(input: &'a str) -> anyhow::Result<Vec<Range>> {
        let mut reader = PlainReader::<&'a [u8]>::from_str(input);

        let mut tag_ranges = Vec::<Range>::new();
        // Stack of the indices of start tag events.
        let mut start_tag_idxs = Vec::<usize>::new();

        let mut start_index = reader.buffer_position();
        let mut start_event_count: usize = 0;
        loop {
            let event = reader.read_event()?;
            match event {
                Event::Start(ref b) => {
                    check_name_len(b.name().as_ref())?;
                    tag_ranges.push(Range {
                        outer_begin: start_index as usize,
                        inner_begin: reader.buffer_position() as usize,
                        inner_end: 0,
                        outer_end: 0,
                    });
                    start_tag_idxs.push(start_event_count);
                    start_event_count += 1;
                }
                Event::End(ref b) => {
                    check_name_len(b.name().as_ref())?;
                    let Some(start_tag_idx) = start_tag_idxs.pop() else {
                        return Err(anyhow::anyhow!("Missing start tag."));
                    };
                    let Some(range) = tag_ranges.get_mut(start_tag_idx) else {
                        return Err(anyhow::anyhow!("Error in range tracking algorithm."));
                    };
                    range.inner_end = start_index as usize;
                    range.outer_end = reader.buffer_position() as usize;
                }
                Event::Empty(ref b) => {
                    check_name_len(b.name().as_ref())?;
                    tag_ranges.push(Range {
                        outer_begin: start_index as usize,
                        inner_begin: 0,
                        inner_end: 0,
                        outer_end: reader.buffer_position() as usize,
                    });
                    start_event_count += 1;
                }
                Event::Eof => {
                    if !start_tag_idxs.is_empty() {
                        return Err(anyhow::anyhow!("Missing end tag."));
                    }
                    return Ok(tag_ranges);
                }
                _ => {}
            }
            start_index = reader.buffer_position();
        }
    }

    /// Creates a new, explicitly configured, reader, reading from the given string.
    pub fn create_with_config(source: &'a str, config: ReaderConfig) -> Self {
        let ranges = if config.xml_substrings { Some(Self::collect_ranges(source)) } else { None };
        let reader = if config.ignore_namespaces {
            let mut plain_reader = PlainReader::from_str(source);
            plain_reader.config_mut().expand_empty_elements = config.expand_empty_elements;
            Reader::Plain(plain_reader)
        } else {
            let mut ns_reader = NsReader::from_str(source);
            ns_reader.config_mut().expand_empty_elements = config.expand_empty_elements;
            Reader::Ns(ns_reader)
        };
        Self {
            reader,
            ranges,
            start_event_count: 0,
            yield_xmlns_attributes: config.yield_xmlns_attributes,
        }
    }

    /// Creates a new reader, reading from the given string.
    pub fn create(source: &'a str) -> Self {
        Self::create_with_config(source, ReaderConfig::default())
    }

    fn update_start_event_count(&mut self, event: &Event<'a>) {
        match &event {
            Event::Start(_) | Event::Empty(_) => {
                self.start_event_count += 1;
            }
            _ => {}
        }
    }

    /// Parses and returns the next event.
    pub fn next_event(&mut self) -> Result<XmlEvent<'a>, XmlError> {
        match self.reader.read_event() {
            Ok(event) => {
                self.update_start_event_count(&event);
                let mut result = XmlEvent {
                    event,
                    resolving_namespaces: match self.reader {
                        Reader::Plain(_) => false,
                        Reader::Ns(_) => true,
                    },
                    yield_xmlns_attributes: self.yield_xmlns_attributes,
                    ..Default::default()
                };
                match &result.event {
                    Event::Start(ref bytes_start) | Event::Empty(ref bytes_start) => {
                        if let Err(e) = check_name_len(bytes_start.name().as_ref()) {
                            return Err(XmlError::invalid_argument(e.to_string()));
                        }
                        (result.local_name_valid, result.prefix_valid) =
                            compute_validity(&bytes_start.name());
                    }
                    Event::End(ref bytes_end) => {
                        if let Err(e) = check_name_len(bytes_end.name().as_ref()) {
                            return Err(XmlError::invalid_argument(e.to_string()));
                        }
                        (result.local_name_valid, result.prefix_valid) =
                            compute_validity(&bytes_end.name());
                    }
                    Event::GeneralRef(ref bytes_ref) => {
                        result.expanded_entity = expand(bytes_ref.deref());
                    }
                    _ => {}
                };
                Ok(result)
            }
            Err(err) => {
                let code = match err {
                    QuickXmlError::Io(_) => XmlErrorCode::Internal,
                    _ => XmlErrorCode::InvalidArgument,
                };
                Err(XmlError::new(err.to_string(), code))
            }
        }
    }

    /// Returns the range of the subtree rooted at the last current event that was a start tag.
    /// The range is [start, end), i.e., the start index belongs to the range, but the end index
    /// does not.
    pub fn subtree_range(&self) -> Result<Option<Range>, XmlError> {
        let Some(ranges) = &self.ranges else {
            return Ok(None);
        };
        let Ok(ranges) = ranges else {
            return Err(XmlError::invalid_argument(ranges.as_ref().unwrap_err().to_string()));
        };
        if self.start_event_count == 0 {
            return Err(XmlError::failed_precondition("Have not read a start tag (yet)."));
        }
        if self.start_event_count > ranges.len() {
            return Err(XmlError::internal("Missing data, undercounted start tags."));
        }
        Ok(Some(ranges[self.start_event_count - 1]))
    }

    /// Resolves the given qualified name to a namespace URI + the local name.
    /// Handling differs for attributes and tag names.
    fn resolve(&self, qname_data: &[u8], is_attribute: bool) -> Result<ResolvedName, XmlError> {
        let Reader::Ns(ns_reader) = &self.reader else {
            return Err(XmlError::failed_precondition("Namespace resolution disabled."));
        };
        let qname = QName(qname_data);
        let (resolve_result, local) = ns_reader.resolver().resolve(qname, !is_attribute);
        match resolve_result {
            ResolveResult::Unbound => Err(XmlError::invalid_argument("Unbound name.")),
            ResolveResult::Unknown(_) => Err(XmlError::invalid_argument("Unknown name.")),
            ResolveResult::Bound(ns) => Ok(ResolvedName {
                namespace_uri: ns.0.into(),
                local_name: local.into_inner().into(),
            }),
        }
    }

    /// Resolves the given attribute name to a namespace URI + the local name.
    pub fn resolve_attribute(&self, qname_data: &[u8]) -> Result<ResolvedName, XmlError> {
        self.resolve(qname_data, true)
    }

    /// Resolves the given tag name to a namespace URI + the local name.
    pub fn resolve_tag(&self, qname_data: &[u8]) -> Result<ResolvedName, XmlError> {
        self.resolve(qname_data, false)
    }
}

/// A reader that wraps a byte slice and replaces any byte with a value > 127
/// with a space (ASCII 32).
struct AsciiSanitizingReader<'a> {
    inner: &'a [u8],
    pos: usize,
}

impl<'a> AsciiSanitizingReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        AsciiSanitizingReader { inner: bytes, pos: 0 }
    }
}

impl<'a> io::Read for AsciiSanitizingReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Don't read past the end of the inner slice
        let bytes_to_process = std::cmp::min(buf.len(), self.inner.len() - self.pos);

        if bytes_to_process == 0 {
            return Ok(0);
        }

        let source_slice = &self.inner[self.pos..self.pos + bytes_to_process];

        for (i, &byte) in source_slice.iter().enumerate() {
            buf[i] = if byte > 127 { b' ' } else { byte };
        }

        self.pos += bytes_to_process;
        Ok(bytes_to_process)
    }
}

/// Extracts the declared encoding from an XML string
///
/// This function parses the provided XML data and looks for the XML declaration
/// (e.g., `<?xml version="1.0" encoding="UTF-8"?>`). This assumes that the input is in some
/// ASCII-compatible encoding. It won't work, e.g., for UTF-16.
///
/// To handle potentially non-UTF8 input for `quick_xml::Reader::from_str`, non-ASCII bytes
/// (values > 127) in `input` are replaced with spaces (' ') before parsing. This allows
/// `quick-xml` to parse the ASCII-based XML declaration, which typically contains the encoding
/// information.
///
/// Returns the encoding value bytes (e.g., `b"UTF-8"`), or `None` if there is no XML declaration
/// or no encoding attribute.
pub fn get_xml_encoding(input: &[u8]) -> Option<Vec<u8>> {
    let sanitizing_reader = AsciiSanitizingReader::new(input);
    let mut reader = PlainReader::from_reader(io::BufReader::new(sanitizing_reader));
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(_) => {
                return None;
            }
            Ok(Event::Decl(e)) => {
                return e.encoding().and_then(Result::ok).map(|cow| cow.into_owned());
            }
            // The XML declaration must appear before the root element.
            // If we find a start tag or EOF, we can stop.
            Ok(Event::Start(_)) | Ok(Event::Empty(_)) | Ok(Event::Eof) => return None,
            _ => {}
        }
        buf.clear();
    }
}
