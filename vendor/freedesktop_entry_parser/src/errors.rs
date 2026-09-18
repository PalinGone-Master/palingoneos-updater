/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use nom::error::{Error as NomError, ErrorKind};
use std::error::Error;
use std::fmt::{Debug, Display};
use std::str::Utf8Error;

/// An error that occurred while parsing. This is the general error type for
/// this library.
#[derive(Debug)]
pub enum ParseError {
    /// Parser encountered some other error.
    /// This is probably the most common error.
    Other {
        /// Remain input when error occurred.
        at: ErrorBytes,
        /// Type of nom parser error.
        kind: ErrorKind,
    },
    /// Parser couldn't finish due to incomplete input.
    Incomplete,
    /// A UTF-8 encoding error.
    Utf8Error { bytes: Vec<u8>, source: Utf8Error },
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Other { at, kind } => {
                write!(f, "Error parings input: {} at {at}", kind.description())
            }
            ParseError::Incomplete => write!(f, "Incomplete input"),
            ParseError::Utf8Error { source, .. } => Display::fmt(&source, f),
        }
    }
}
impl Error for ParseError {}

/// The remaining input from the parser.  Useful for debugging to see where the
/// parser failed.  This is used in [`ParseError`](struct.ParseError.html).
/// It'll be `Valid` if the remaining input was a valid string and `Invalid` if
/// it wasn't
#[derive(Debug)]
pub enum ErrorBytes {
    /// Input was a valid string
    Valid(String),
    /// Input was not a valid string
    Invalid(Vec<u8>),
}

impl Display for ErrorBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorBytes::Valid(str) => Display::fmt(&str, f),
            ErrorBytes::Invalid(bytes) => Debug::fmt(&bytes, f),
        }
    }
}

impl Error for ErrorBytes {}

impl From<nom::Err<NomError<&[u8]>>> for ParseError {
    fn from(e: nom::Err<NomError<&[u8]>>) -> Self {
        match e {
            nom::Err::Error(NomError { input, code })
            | nom::Err::Failure(NomError { input, code }) => {
                match std::str::from_utf8(input) {
                    Ok(s) => ParseError::Other {
                        at: ErrorBytes::Valid(s.to_owned()),
                        kind: code,
                    },
                    Err(_) => ParseError::Other {
                        at: ErrorBytes::Invalid(input.to_vec()),
                        kind: code,
                    },
                }
            }
            nom::Err::Incomplete(_) => ParseError::Incomplete,
        }
    }
}
