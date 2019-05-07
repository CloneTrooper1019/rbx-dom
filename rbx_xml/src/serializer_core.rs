use std::{
    fmt::Write as FmtWrite,
    io::{self, Write},
};

use failure::Fail;
use xml::writer::{self, EventWriter, EmitterConfig};

pub use xml::writer::XmlEvent as XmlWriteEvent;

#[derive(Debug, Fail)]
pub enum EncodeError {
    #[fail(display = "IO Error: {}", _0)]
    IoError(#[fail(cause)] io::Error),

    #[fail(display = "XML error: {}", _0)]
    XmlError(#[fail(cause)] writer::Error),

    #[fail(display = "{}", _0)]
    Message(&'static str),

    #[doc(hidden)]
    #[fail(display = "<this variant should never exist>")]
    __Nonexhaustive,
}

impl From<xml::writer::Error> for EncodeError {
    fn from(error: xml::writer::Error) -> EncodeError {
        match error {
            xml::writer::Error::Io(inner) => EncodeError::IoError(inner),
            _ => EncodeError::XmlError(error),
        }
    }
}

/// A wrapper around an xml-rs `EventWriter` as well as other state kept around
/// for performantly emitting XML.
pub struct XmlEventWriter<W> {
    inner: EventWriter<W>,
    character_buffer: String,
}

impl<W: Write> XmlEventWriter<W> {
    /// Constructs an `XmlEventWriter` from an output that implements `Write`.
    pub fn from_output(output: W) -> XmlEventWriter<W> {
        let inner = EmitterConfig::new()
            .perform_indent(true)
            .write_document_declaration(false)
            .normalize_empty_elements(false)
            .create_writer(output);

        XmlEventWriter {
            inner,
            character_buffer: String::new(),
        }
    }

    /// Writes a single XML event to the output stream.
    pub fn write<'a, E>(&mut self, event: E) -> Result<(), writer::Error>
        where E: Into<XmlWriteEvent<'a>>
    {
        self.inner.write(event)
    }

    /// Writes a string slice to the output stream as characters or CDATA.
    pub fn write_string(&mut self, value: &str) -> Result<(), writer::Error> {
        write_characters_or_cdata(&mut self.inner, value)
    }

    /// Writes a value that implements `Display` as characters or CDATA. Resuses
    /// an internal buffer to avoid unnecessary allocations.
    pub fn write_characters<T: std::fmt::Display>(&mut self, value: T) -> Result<(), writer::Error> {
        write!(self.character_buffer, "{}", value).unwrap();
        write_characters_or_cdata(&mut self.inner, &self.character_buffer)?;
        self.character_buffer.clear();

        Ok(())
    }

    /// The same as `write_characters`, but wraps the characters in a tag with
    /// the given name and no attributes.
    pub fn write_tag_characters<T: std::fmt::Display>(&mut self, tag: &str, value: T) -> Result<(), writer::Error> {
        self.write(XmlWriteEvent::start_element(tag))?;
        self.write_characters(value)?;
        self.write(XmlWriteEvent::end_element())
    }

    /// Writes a list of values that implement `Display`, with each wrapped in
    /// an associated tag. This method uses the same optimization as
    /// `write_characters` to avoid extra allocations.
    pub fn write_tag_array<T: std::fmt::Display>(&mut self, values: &[T], tags: &[&str]) -> Result<(), writer::Error> {
        assert_eq!(values.len(), tags.len());

        for (index, component) in values.iter().enumerate() {
            self.write_tag_characters(tags[index], component)?;
        }

        Ok(())
    }
}

/// Given a value, writes a `Characters` event or a `CData` event depending on
/// whether the input string contains whitespace that needs to be explicitly
/// preserved.
///
/// This method is extracted so that it can be used inside both `write_string`
/// and `write_characters` without borrowing issues.
fn write_characters_or_cdata<W: Write>(writer: &mut EventWriter<W>, value: &str) -> Result<(), writer::Error> {
    let first_char = value.chars().next();
    let last_char = value.chars().next_back();

    // If the string has leading or trailing whitespace, we switch to
    // writing it as part of a CDATA block instead of a regular characters
    // block.
    let has_outer_whitespace = match (first_char, last_char) {
        (Some(first), Some(last)) => first.is_whitespace() || last.is_whitespace(),
        (Some(char), None) | (None, Some(char)) => char.is_whitespace(),
        (None, None) => false,
    };

    if has_outer_whitespace {
        writer.write(XmlWriteEvent::cdata(value))?;
    } else {
        writer.write(XmlWriteEvent::characters(value))?;
    }

    Ok(())
}