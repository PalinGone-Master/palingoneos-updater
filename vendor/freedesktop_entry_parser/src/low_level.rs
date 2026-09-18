/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The low-level API is a less ergonomic but is a mostly zero-copy. It exposes a byte oriented
//! API and a string oriented API. They are identical expect that the string API also checks if all
//! the byte strings are valid UTF-8. Both APIs return an iterator over the sections in the file.
//! The sections are returned in the order they appear in the file.
//!
//! This API is mostly zero-copy, meaning that is doesn't copy any of the input data while
//! parsing. With the exception of values in key value pairs that are spread across multiple lines.
//! These values must be copied to produce a single output slice without modifying the input.
//!
//! If you wish to just use this API you can disable to high-level API and remove it's dependency
//! [`indexmap`] by setting `default-features = false` on `freedesktop_entry_parser` in your
//! `Cargo.toml`.
//!
//! Byte API Example:
//! ```
//! use freedesktop_entry_parser::low_level::{parse_entry, SectionBytes, AttrBytes, ParamBytes};
//! use std::borrow::Cow;
//!
//! let file = b"[Desktop Entry]
//! Name=Firefox
//! GenericName=Web Browser
//! GenericName[no]=Nettleser
//! Exec=firefox %u
//! Icon=firefox";
//!
//! assert_eq!(parse_entry(file).next().unwrap()?, SectionBytes {
//!     title: b"Desktop Entry",
//!     attrs: vec![
//!         AttrBytes { name: b"Name", value: Cow::from(&b"Firefox"[..]), param: None},
//!         AttrBytes { name: b"GenericName", value: Cow::from(&b"Web Browser"[..]), param: None},
//!         AttrBytes { name: b"GenericName[no]", value: Cow::from(&b"Nettleser"[..]), param: Some(
//!             ParamBytes { param: b"no", attr_name: b"GenericName" }
//!         )},
//!         AttrBytes { name: b"Exec", value: Cow::from(&b"firefox %u"[..]), param: None},
//!         AttrBytes { name: b"Icon", value: Cow::from(&b"firefox"[..]), param: None},
//!     ]
//! });
//! # Ok::<(), freedesktop_entry_parser::low_level::ParseError>(())
//! ```
//!
//! String API Example:
//! ```
//! use freedesktop_entry_parser::low_level::{parse_entry_str, SectionStr, AttrStr, ParamStr};
//! use std::borrow::Cow;
//!
//! let file = b"[Desktop Entry]
//! Name=Firefox
//! GenericName=Web Browser
//! GenericName[no]=Nettleser
//! Exec=firefox %u
//! Icon=firefox";
//!
//! assert_eq!(parse_entry_str(file).next().unwrap()?, SectionStr {
//!     title: "Desktop Entry",
//!     attrs: vec![
//!         AttrStr { name: "Name", value: Cow::from("Firefox"), param: None},
//!         AttrStr { name: "GenericName", value: Cow::from("Web Browser"), param: None},
//!         AttrStr { name: "GenericName[no]", value: Cow::from("Nettleser"), param: Some(
//!             ParamStr { param: "no", attr_name: "GenericName" }
//!         )},
//!         AttrStr { name: "Exec", value: Cow::from("firefox %u"), param: None},
//!         AttrStr { name: "Icon", value: Cow::from("firefox"), param: None},
//!     ]
//! });
//! # Ok::<(), freedesktop_entry_parser::low_level::ParseError>(())
//! ```

pub use crate::errors::ParseError;
use nom::bytes::complete::{tag, take_till, take_till1};
use nom::error::ErrorKind;
use nom::multi::many1;
use nom::sequence::{delimited, terminated};
use nom::{IResult, Parser};
use std::borrow::Cow;
use std::fmt::Debug;
use std::iter::Iterator;
use std::str::from_utf8;

/// A name and value pair from a [`SectionBytes`].
#[derive(PartialEq, Eq)]
pub struct AttrBytes<'a> {
    /// Name of the attribute.
    pub name: &'a [u8],
    /// Value of the attribute. This value is a [`Cow`] because it might have been copied into an
    /// owned buffer if it was spread across multiple lines with a `\`.
    pub value: Cow<'a, [u8]>,
    /// Some attributes have a parameter like `GenericName[es]=Navegador web`.
    /// If it does this field will be present.
    /// In those cases `name` will contain the entire name left of the equal sign.
    pub param: Option<ParamBytes<'a>>,
}

/// A param value and attribute name that may be a part of a [`AttrBytes`].
#[derive(PartialEq, Eq)]
pub struct ParamBytes<'a> {
    /// Value of the the param, ex. `es`.
    pub param: &'a [u8],
    /// Name of the attribute without the param ex `GenericName`.
    pub attr_name: &'a [u8],
}

/// One section on a entry file.
#[derive(PartialEq, Eq)]
pub struct SectionBytes<'a> {
    /// Section title.
    pub title: &'a [u8],
    /// List of attributes.
    pub attrs: Vec<AttrBytes<'a>>,
}

impl<'a> Debug for AttrBytes<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match from_utf8(self.name) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("{:?}", self.name),
        };
        let value = match from_utf8(&self.value) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("{:?}", self.value),
        };
        f.debug_struct("AttrBytes")
            .field("name", &name)
            .field("value", &value)
            .field("param", &self.param)
            .finish()
    }
}

impl<'a> Debug for SectionBytes<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = match from_utf8(self.title) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("{:?}", self.title),
        };
        f.debug_struct("SectionBytes")
            .field("title", &title)
            .field("attrs", &self.attrs)
            .finish()
    }
}

impl<'a> Debug for ParamBytes<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let attr_name = match from_utf8(self.attr_name) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("{:?}", self.attr_name),
        };
        let param = match from_utf8(self.param) {
            Ok(s) => s.to_owned(),
            Err(_) => format!("{:?}", self.param),
        };
        f.debug_struct("AttrBytes")
            .field("attr_name", &attr_name)
            .field("param", &param)
            .finish()
    }
}

/// A name and value pair from a [`SectionStr`].
#[derive(Debug, PartialEq, Eq)]
pub struct AttrStr<'a> {
    /// Name of the attribute.
    pub name: &'a str,
    /// Value of the attribute. This value is a [`Cow`] because it might have been copied into an
    /// owned buffer if it was spread across multiple lines with a `\`.
    pub value: Cow<'a, str>,
    /// Some attributes have a parameter like `GenericName[es]=Navegador web`.
    /// If it does this field will be present.
    /// In those cases `name` will contain the entire name left of the equal sign.
    pub param: Option<ParamStr<'a>>,
}

/// A param value and attribute name that may be a part of a [`AttrStr`].
#[derive(Debug, PartialEq, Eq)]
pub struct ParamStr<'a> {
    /// Value of the the param, ex. `es`.
    pub param: &'a str,
    /// Name of the attribute without the param ex `GenericName`.
    pub attr_name: &'a str,
}

/// One section on a entry file.
#[derive(Debug, PartialEq, Eq)]
pub struct SectionStr<'a> {
    /// Section title.
    pub title: &'a str,
    /// List of attributes.
    pub attrs: Vec<AttrStr<'a>>,
}

#[inline]
fn parse_str(input: &[u8]) -> Result<&str, ParseError> {
    std::str::from_utf8(input).map_err(|e| ParseError::Utf8Error {
        bytes: input.to_owned(),
        source: e,
    })
}

#[inline]
fn parse_string(input: Vec<u8>) -> Result<String, ParseError> {
    String::from_utf8(input).map_err(|e| {
        let source = e.utf8_error();
        ParseError::Utf8Error {
            bytes: e.into_bytes(),
            source,
        }
    })
}

impl<'a> TryFrom<AttrBytes<'a>> for AttrStr<'a> {
    type Error = ParseError;

    fn try_from(value: AttrBytes<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            name: parse_str(value.name)?,
            value: match value.value {
                Cow::Borrowed(s) => Cow::Borrowed(parse_str(s)?),
                Cow::Owned(s) => Cow::Owned(parse_string(s)?),
            },
            param: match value.param {
                Some(param) => Some(param.try_into()?),
                None => None,
            },
        })
    }
}

impl<'a> TryFrom<ParamBytes<'a>> for ParamStr<'a> {
    type Error = ParseError;

    fn try_from(value: ParamBytes<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            param: parse_str(value.param)?,
            attr_name: parse_str(value.attr_name)?,
        })
    }
}

impl<'a> TryFrom<SectionBytes<'a>> for SectionStr<'a> {
    type Error = ParseError;

    fn try_from(value: SectionBytes<'a>) -> Result<Self, Self::Error> {
        Ok(Self {
            title: parse_str(value.title)?,
            attrs: value
                .attrs
                .into_iter()
                .map(|attrs| attrs.try_into())
                .collect::<Result<Vec<AttrStr<'a>>, Self::Error>>()?,
        })
    }
}

fn not_whitespace(c: u8) -> bool {
    c != b'\n' && c != b'\t' && c != b'\r' && c != b' '
}

/// Parse a header line, return the header name.
fn header(input: &[u8]) -> IResult<&[u8], &[u8]> {
    delimited(tag("["), take_till1(|c| c == b']' || c == b'['), tag("]"))
        .parse(input)
}

/// Does input start with a comment character
fn comment_line(input: &[u8]) -> bool {
    input.first() == Some(&(b'#')) || input.first() == Some(&(b';'))
}

/// Find the next line, ignoring comments.
fn next_line(
    input: &[u8],
) -> Result<&[u8], nom::Err<nom::error::Error<&[u8]>>> {
    if input.is_empty() {
        return Ok(b"");
    }
    let (rem, _) = take_till(not_whitespace)(input)?;
    if comment_line(rem) {
        let (rem, _) = take_till(|c| c == b'\n')(rem)?;
        return next_line(rem);
    }
    Ok(rem)
}

fn find_start(input: &[u8]) -> IResult<&[u8], &[u8]> {
    take_till(|c| c == b'[')(input)
}

fn trim_whitespace_front(input: &[u8]) -> &[u8] {
    match input.iter().position(|c| not_whitespace(*c)) {
        Some(first_non_whitespace) => &input[first_non_whitespace..],
        None => &[],
    }
}

fn trim_whitespace_back(input: &[u8]) -> &[u8] {
    match input.iter().rposition(|c| not_whitespace(*c)) {
        Some(last_non_whitespace) => &input[..last_non_whitespace + 1],
        None => &[],
    }
}

/// Parse attr params.
fn params(input: &[u8]) -> IResult<&[u8], ParamBytes<'_>> {
    let (rem, attr_name) =
        terminated(take_till(|c| c == b'['), tag("[")).parse(input)?;
    let (rem, param) = take_till(|c| c == b']')(rem)?;
    Ok((rem, ParamBytes { param, attr_name }))
}

#[derive(PartialEq, Eq)]
enum LineCont {
    End,
    Cont,
}

fn value_line(input: &[u8]) -> IResult<&[u8], (LineCont, &[u8])> {
    let (rem, line) = take_till(|c| c == b'\n' || c == b'\\')(input)?;
    match rem.first() {
        Some(b'\\') => Ok((next_line(&rem[1..])?, (LineCont::Cont, line))),
        _ => Ok((next_line(rem)?, (LineCont::End, line))),
    }
}

fn value(input: &[u8]) -> IResult<&[u8], Cow<'_, [u8]>> {
    let (rem, (cont, line)) = value_line(input)?;
    let line = trim_whitespace_front(line);
    match cont {
        LineCont::End => Ok((rem, line.into())),
        LineCont::Cont => {
            let mut line = Vec::from(line);
            line.push(b' '); // Add space to replace backslash
            let mut rem = rem;
            loop {
                let (rem_next, (cont, line_part)) = value_line(rem)?;
                line.extend(line_part);
                rem = rem_next;
                match cont {
                    LineCont::End => break,
                    LineCont::Cont => line.push(b' '), // Add space to replace backslash
                }
            }
            Ok((rem, line.into()))
        }
    }
}

fn attr(input: &[u8]) -> IResult<&[u8], AttrBytes<'_>> {
    if input.first() == Some(&(b'[')) {
        return Err(nom::Err::Error(nom::error::Error::new(
            input,
            ErrorKind::Complete,
        )));
    }
    let (rem, name) =
        terminated(take_till(|c| c == b'='), tag("=")).parse(input)?;
    let name = trim_whitespace_back(name);
    let (rem, value) = value(rem)?;

    Ok((
        next_line(rem)?,
        AttrBytes {
            name,
            value,
            param: params(name).ok().map(|(_, param)| param),
        },
    ))
}

fn section(input: &[u8]) -> IResult<&[u8], SectionBytes<'_>> {
    let (rem, title) = header(input)?;
    let rem = next_line(rem)?;
    let (rem, attrs) = many1(attr).parse(rem)?;
    Ok((rem, SectionBytes { title, attrs }))
}

/// An iterator over the sections in a entry file as bytes. Returns an instance of [`SectionStr`]
/// for each successfully parsed section or an instance of [`ParseError`] for sections that
/// fail to parse. After an error, the iterator will stop.
pub struct SectionBytesIter<'a> {
    /// Remaining bytes to parse.
    rem: &'a [u8],
    /// Has the start of file been found.
    found_start: bool,
    /// Has an error been encountered.
    error: bool,
}

impl<'a> SectionBytesIter<'a> {
    fn next_section(&mut self) -> Result<SectionBytes<'a>, ParseError> {
        self.rem = find_start(self.rem)?.0;
        self.found_start = true;
        let (rem, section_bytes) = section(self.rem)?;
        self.rem = rem;
        Ok(section_bytes)
    }
}

impl<'a> Iterator for SectionBytesIter<'a> {
    type Item = Result<SectionBytes<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rem.is_empty() || self.error {
            return None;
        }
        let next = self.next_section();
        self.error = next.is_err();
        Some(next)
    }
}

/// Parse a FreeDesktop entry file as bytes.
///
/// Returns and iterator over the sections in the file.
///
/// # Example
///
/// ```
/// use freedesktop_entry_parser::low_level::parse_entry;
///
/// let entry_bytes = b"
/// [Unit]
/// Description=OpenSSH Daemon
/// Wants=sshdgenkeys.service
/// After=sshdgenkeys.service
/// After=network.target
///
/// [Service]
/// ExecStart=/usr/bin/sshd -D
/// ExecReload=/bin/kill -HUP $MAINPID
/// KillMode=process
/// Restart=always
///
/// [Install]
/// WantedBy=multi-user.target
/// ";
/// let start_cmd = parse_entry(entry_bytes)
///     .filter_map(Result::ok) // Filter out sections that failed to parse
///     .find(|section| section.title == b"Service")
///     .unwrap() // Assume "Service" section was found
///     .attrs
///     .into_iter()
///     .find(|attr| attr.name == b"ExecStart")
///     .unwrap(); // Assume "ExecStart" attribute was found
/// assert_eq!(start_cmd.value.as_ref(), b"/usr/bin/sshd -D");
/// # Ok::<(), freedesktop_entry_parser::ParseError>(())
/// ```
pub fn parse_entry(input: &[u8]) -> SectionBytesIter<'_> {
    SectionBytesIter {
        rem: input,
        found_start: false,
        error: false,
    }
}

/// An iterator over the sections in a entry file as strings. Returns an instance of [`SectionStr`]
/// for each successfully parsed section or an instance of [`ParseError`] for sections that
/// fail to parse. After an error, the iterator will stop.
pub struct SectionStrIter<'a> {
    internal: SectionBytesIter<'a>,
}

impl<'a> Iterator for SectionStrIter<'a> {
    type Item = Result<SectionStr<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.internal.next() {
            Some(Ok(v)) => Some(SectionStr::try_from(v)),
            Some(Err(e)) => Some(Err(e)),
            None => None,
        }
    }
}

/// Parse a FreeDesktop entry file as strings.
///
/// Returns and iterator over the sections in the file.
///
/// # Example
///
/// ```
/// use freedesktop_entry_parser::low_level::parse_entry_str;
///
/// let entry_bytes = b"
/// [Unit]
/// Description=OpenSSH Daemon
/// Wants=sshdgenkeys.service
/// After=sshdgenkeys.service
/// After=network.target
///
/// [Service]
/// ExecStart=/usr/bin/sshd -D
/// ExecReload=/bin/kill -HUP $MAINPID
/// KillMode=process
/// Restart=always
///
/// [Install]
/// WantedBy=multi-user.target
/// ";
/// let start_cmd = parse_entry_str(entry_bytes)
///     .filter_map(Result::ok) // Filter out sections that failed to parse
///     .find(|section| section.title == "Service")
///     .unwrap() // Assume "Service" section was found
///     .attrs
///     .into_iter()
///     .find(|attr| attr.name == "ExecStart")
///     .unwrap(); // Assume "ExecStart" attribute was found
/// assert_eq!(start_cmd.value.as_ref(), "/usr/bin/sshd -D");
/// # Ok::<(), freedesktop_entry_parser::ParseError>(())
/// ```
pub fn parse_entry_str(input: &[u8]) -> SectionStrIter<'_> {
    SectionStrIter {
        internal: SectionBytesIter {
            rem: input,
            found_start: false,
            error: false,
        },
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn trim_front() {
        assert_eq!(trim_whitespace_front(b" \ttest"), &b"test"[..])
    }

    #[test]
    fn trim_back() {
        assert_eq!(trim_whitespace_back(b"test  \t"), &b"test"[..])
    }

    #[test]
    fn trim_front_none() {
        assert_eq!(trim_whitespace_front(b"test"), &b"test"[..])
    }

    #[test]
    fn trim_back_none() {
        assert_eq!(trim_whitespace_back(b"test"), &b"test"[..])
    }

    mod fn_header {
        use super::*;

        #[test]
        fn ok() {
            assert_eq!(header(b"[hello]"), Ok((&b""[..], &b"hello"[..])));
        }

        #[test]
        fn no_start() {
            assert_eq!(
                header(b"hello").unwrap_err(),
                nom::Err::Error(nom::error::make_error(
                    &b"hello"[..],
                    ErrorKind::Tag
                ))
            );
        }

        #[test]
        fn no_end() {
            assert_eq!(
                header(b"[hello").unwrap_err(),
                nom::Err::Error(nom::error::make_error(
                    &b""[..],
                    ErrorKind::Tag
                ))
            );
        }

        #[test]
        fn double_start_bracket() {
            assert_eq!(
                header(b"[h[ello]").unwrap_err(),
                nom::Err::Error(nom::error::make_error(
                    &b"[ello]"[..],
                    ErrorKind::Tag
                ))
            );
        }
    }

    mod fn_next_line {
        use super::*;
        #[test]
        fn empty() {
            assert_eq!(next_line(b""), Ok(&b""[..]));
        }

        #[test]
        fn only_whitespace() {
            assert_eq!(next_line(b" \t \t\n\r\nhello"), Ok(&b"hello"[..]));
        }

        #[test]
        fn comment() {
            assert_eq!(
                next_line(b"   \t\n# Comment\nhello"),
                Ok(&b"hello"[..])
            );
        }

        #[test]
        fn no_change() {
            assert_eq!(next_line(b"hello\n"), Ok(&b"hello\n"[..]));
        }
    }

    mod fn_attr {
        use super::*;

        #[test]
        fn ok() {
            assert_eq!(
                attr(b"hello=world"),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b"hello"[..],
                        value: b"world"[..].into(),
                        param: None,
                    }
                ))
            );
        }

        #[test]
        fn with_param() {
            assert_eq!(
                attr(b"hello[en]=world"),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b"hello[en]"[..],
                        value: b"world"[..].into(),
                        param: Some(ParamBytes {
                            attr_name: &b"hello"[..],
                            param: &b"en"[..]
                        }),
                    }
                ))
            );
        }

        #[test]
        fn space_in_value() {
            assert_eq!(
                attr(b"hello=world today"),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b"hello"[..],
                        value: b"world today"[..].into(),
                        param: None,
                    }
                ))
            );
        }

        #[test]
        fn no_value() {
            assert_eq!(
                attr(b"hello="),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b"hello"[..],
                        value: b""[..].into(),
                        param: None,
                    }
                ))
            );
        }

        #[test]
        fn no_name() {
            assert_eq!(
                attr(b"=world"),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b""[..],
                        value: b"world"[..].into(),
                        param: None,
                    }
                ))
            );
        }

        #[test]
        fn no_eq() {
            assert_eq!(
                attr(b"hello"),
                Err(nom::Err::Error(nom::error::Error {
                    input: &b""[..],
                    code: ErrorKind::Tag
                }))
            );
        }

        #[test]
        fn whitespace() {
            assert_eq!(
                attr(b"hello = world today"),
                Ok((
                    &b""[..],
                    AttrBytes {
                        name: &b"hello"[..],
                        value: b"world today"[..].into(),
                        param: None,
                    }
                ))
            );
        }
    }

    mod fn_section {
        use super::*;

        #[test]
        fn ok() {
            assert_eq!(
                section(b"[apps]\nSize=48\nScale=1"),
                Ok((
                    &b""[..],
                    SectionBytes {
                        title: &b"apps"[..],
                        attrs: vec![
                            AttrBytes {
                                name: &b"Size"[..],
                                value: b"48"[..].into(),
                                param: None,
                            },
                            AttrBytes {
                                name: &b"Scale"[..],
                                value: b"1"[..].into(),
                                param: None,
                            }
                        ]
                    }
                ))
            );
        }

        #[test]
        fn no_attrs() {
            assert_eq!(
                section(b"[apps]\n"),
                Err(nom::Err::Error(nom::error::Error {
                    input: &b""[..],
                    code: ErrorKind::Tag
                }))
            );
        }

        #[test]
        fn no_header() {
            assert_eq!(
                section(b"Size=48\nScale=1"),
                Err(nom::Err::Error(nom::error::Error {
                    input: &b"Size=48\nScale=1"[..],
                    code: ErrorKind::Tag
                }))
            );
        }
    }

    mod fn_value {

        use super::*;

        #[test]
        fn single_line() {
            assert_eq!(value(b"value\n"), Ok((&b""[..], b"value"[..].into())))
        }

        #[test]
        fn two_line() {
            assert_eq!(
                value(b"value\\\nvalue2\n"),
                Ok((&b""[..], b"value value2"[..].into()))
            )
        }

        #[test]
        fn three_line() {
            assert_eq!(
                value(b"value\\\nvalue2\\\nvalue3\n"),
                Ok((&b""[..], b"value value2 value3"[..].into()))
            )
        }

        #[test]
        fn three_line_one_comment() {
            assert_eq!(
                value(b"value\\\nvalue2\\\n# commnet\nvalue3\n"),
                Ok((&b""[..], b"value value2 value3"[..].into()))
            )
        }

        #[test]
        fn three_line_two_comment() {
            assert_eq!(
                value(b"value\\\nvalue2\\\n# commnet\n; comment 2\nvalue3\n"),
                Ok((&b""[..], b"value value2 value3"[..].into()))
            )
        }
    }

    #[test]
    fn parse_icon_index() {
        let input = include_bytes!("./../test_data/gnome-index.theme");
        let sections = parse_entry(input)
            .collect::<Result<Vec<_>, _>>()
            .expect("Error parsing input");
        assert_eq!(sections.len(), 68);
        assert_eq!(sections[50].title, &b"48x48/status"[..]);
        assert_eq!(sections[50].attrs[1].name, &b"Size"[..]);
        assert_eq!(sections[50].attrs[1].value, &b"48"[..]);
    }

    #[test]
    fn parse_firefox_desktop_entry() {
        let input = include_bytes!("./../test_data/firefox.desktop");
        let sections = parse_entry(input)
            .collect::<Result<Vec<_>, _>>()
            .expect("Error parsing input");
        assert_eq!(sections.len(), 3);
        assert_eq!(
            sections[0].attrs[1],
            AttrBytes {
                name: &b"Name"[..],
                value: b"Firefox"[..].into(),
                param: None,
            }
        );
        assert_eq!(
            sections[0].attrs[4],
            AttrBytes {
                name: &b"GenericName[ast]"[..],
                value: b"Restolador Web"[..].into(),
                param: Some(ParamBytes {
                    attr_name: &b"GenericName"[..],
                    param: &b"ast"[..]
                }),
            }
        );
    }

    #[test]
    fn parse_sshd_systemd_unit() {
        let input = include_bytes!("./../test_data/sshd.service");
        let sections = parse_entry(input)
            .collect::<Result<Vec<_>, _>>()
            .expect("Error parsing input");
        assert_eq!(sections.len(), 3);
    }

    #[test]
    fn parse_systemd_test() {
        let input = include_bytes!("./../test_data/edge-cases.txt");
        let sections = parse_entry(input)
            .collect::<Result<Vec<_>, _>>()
            .expect("Error parsing input");
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, &b"Section A"[..]);
        assert_eq!(sections[0].attrs[0].name, &b"KeyOne"[..]);
        assert_eq!(sections[0].attrs[0].value, &b"value 1"[..]);
        assert_eq!(sections[0].attrs[1].name, &b"KeyTwo"[..]);
        assert_eq!(sections[0].attrs[1].value, &b"value 2"[..]);

        assert_eq!(sections[1].title, &b"Section B"[..]);
        assert_eq!(sections[1].attrs[0].name, &b"Setting"[..]);
        assert_eq!(sections[1].attrs[0].value, &b"\"something\" \"some thing\" \"\xE2\x80\xA6\""[..]);
        assert_eq!(sections[1].attrs[1].name, &b"KeyTwo"[..]);
        assert_eq!(sections[1].attrs[1].value, &b"value 2 value 2 continued"[..]);

        assert_eq!(sections[2].title, &b"Section C"[..]);
        assert_eq!(sections[2].attrs[0].name, &b"KeyThree"[..]);
        assert_eq!(sections[2].attrs[0].value, &b"value 3 value 3 continued"[..]);
    }
}
