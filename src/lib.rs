//! A small JSON parser with borrowed strings and lossless numbers.
//!
//! ```
//! let value = strata_json::from_str(r#"{"name":"Ada","age":37}"#)?;
//! assert_eq!(value.get("name").and_then(|v| v.as_str()), Some("Ada"));
//! assert_eq!(value.get("age").and_then(|v| v.as_number()).and_then(|n| n.as_u64()), Some(37));
//! # Ok::<(), strata_json::Error>(())
//! ```
//! Unescaped strings and all numbers borrow the input. Arrays, objects, and
//! escaped strings allocate. Keep the input alive while using the parsed value.

mod parser;

use std::{borrow::Cow, fmt};

/// Default and maximum supported number of nested arrays or objects.
pub const MAX_DEPTH: usize = 128;

/// A validated JSON number, retaining its original spelling without rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Number<'a>(pub(crate) &'a str);

impl<'a> Number<'a> {
    /// Returns the original JSON number token.
    pub fn as_str(self) -> &'a str {
        self.0
    }
    /// Converts an integer token to `i64`, returning `None` on overflow or a fraction/exponent.
    pub fn as_i64(self) -> Option<i64> {
        self.0.parse().ok()
    }
    /// Converts an unsigned integer token to `u64`, returning `None` on failure.
    pub fn as_u64(self) -> Option<u64> {
        self.0.parse().ok()
    }
    /// Converts to a finite `f64`. May round or underflow; returns `None` on overflow.
    pub fn as_f64(self) -> Option<f64> {
        self.0.parse::<f64>().ok().filter(|n| n.is_finite())
    }
}

impl fmt::Display for Number<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// A JSON value. Object entries preserve input order and duplicate keys.
/// Equality is structural: object order and number spelling are significant.
#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    /// JSON null.
    Null,
    /// JSON boolean.
    Bool(bool),
    /// A lossless, borrowed numeric token.
    Number(Number<'a>),
    /// Borrowed when unescaped, owned when escape decoding was necessary.
    String(Cow<'a, str>),
    /// Ordered values.
    Array(Vec<Value<'a>>),
    /// Ordered key/value pairs, including duplicates.
    Object(Vec<(Cow<'a, str>, Value<'a>)>),
}

impl<'a> Value<'a> {
    /// Gets the last matching object entry in O(n) time.
    pub fn get(&self, key: &str) -> Option<&Value<'a>> {
        self.as_object()?
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
    /// Returns the string contents if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// Returns the numeric token if this is a number.
    pub fn as_number(&self) -> Option<Number<'a>> {
        if let Self::Number(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    /// Returns the boolean if this is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    /// Returns the elements if this is an array.
    pub fn as_array(&self) -> Option<&[Value<'a>]> {
        if let Self::Array(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// Returns all entries if this is an object.
    pub fn as_object(&self) -> Option<&[(Cow<'a, str>, Value<'a>)]> {
        if let Self::Object(v) = self {
            Some(v)
        } else {
            None
        }
    }
    /// Tests for JSON null.
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// A stable category for a parse failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Input ended before a complete value was read.
    UnexpectedEof,
    /// An unexpected token or delimiter was encountered.
    UnexpectedToken,
    /// Bytes remain after the root value.
    TrailingCharacters,
    /// Invalid JSON number grammar.
    InvalidNumber,
    /// Invalid escape or unescaped control character.
    InvalidString,
    /// Invalid Unicode escape or unpaired UTF-16 surrogate.
    InvalidUnicode,
    /// Input bytes are not UTF-8.
    InvalidUtf8,
    /// The configured nesting limit was exceeded.
    DepthLimit,
}

/// A parse failure with a zero-based byte offset and one-based line/byte-column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Failure category.
    pub kind: ErrorKind,
    /// Byte offset into the original input (or its length at EOF).
    pub offset: usize,
    /// One-based line, counting LF as a line separator.
    pub line: usize,
    /// One-based byte column, not a Unicode character column.
    pub column: usize,
}

impl Error {
    pub(crate) fn new(input: &[u8], offset: usize, kind: ErrorKind) -> Self {
        let prefix = &input[..offset];
        let line = 1 + prefix.iter().filter(|&&b| b == b'\n').count();
        let column = prefix
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(offset + 1, |i| offset - i);
        Self {
            kind,
            offset,
            line,
            column,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ErrorKind::UnexpectedEof => "unexpected end of input",
            ErrorKind::UnexpectedToken => "unexpected token",
            ErrorKind::TrailingCharacters => "trailing characters",
            ErrorKind::InvalidNumber => "invalid number",
            ErrorKind::InvalidString => "invalid string escape or control character",
            ErrorKind::InvalidUnicode => "invalid Unicode escape or surrogate pair",
            ErrorKind::InvalidUtf8 => "invalid UTF-8",
            ErrorKind::DepthLimit => "nesting limit exceeded",
        };
        write!(
            f,
            "{message} at line {}, column {} (byte {})",
            self.line, self.column, self.offset
        )
    }
}
impl std::error::Error for Error {}

/// Parses exactly one JSON value with the default nesting limit.
pub fn from_str(input: &str) -> Result<Value<'_>, Error> {
    from_str_with_depth(input, MAX_DEPTH)
}

/// Parses with a lower nesting limit. Values above [`MAX_DEPTH`] are clamped.
/// A limit of zero permits only scalar values; each array or object adds one.
pub fn from_str_with_depth(input: &str, max_depth: usize) -> Result<Value<'_>, Error> {
    parser::parse(input, max_depth.min(MAX_DEPTH))
}

/// Validates UTF-8, then parses exactly one JSON value, borrowing from `input`.
pub fn from_slice(input: &[u8]) -> Result<Value<'_>, Error> {
    let text = std::str::from_utf8(input)
        .map_err(|e| Error::new(input, e.valid_up_to(), ErrorKind::InvalidUtf8))?;
    from_str(text)
}
