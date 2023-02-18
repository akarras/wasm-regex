//! All the messy-ish error handling code

use crate::utf16_index_bytes;
use crate::utf16_index_chars;
use regex_syntax::ast::Position as RePosition;
use regex_syntax::ast::Span as ReSpan;
use serde::Serialize;
use std::str;

/// Wrapper so we can serialize regex errors
#[derive(Debug, Serialize)]
#[serde(rename_all(serialize = "camelCase"))]
#[serde(tag = "errorClass", content = "error")]
pub enum Error {
    /// An error from regex
    RegexSyntax(Box<ReSyntax>),
    /// Regex compiled larger than the limit (unlikely, unless we set a limit)
    RegexCompiledTooBig(String),
    /// Unspecified error (very unlikely)
    RegexUnspecified(String),
    /// Some sort of invalid replacement
    Encoding(String),
}

/// Add automatic conversion from regex error to our error type
impl From<regex::Error> for Error {
    fn from(value: regex::Error) -> Self {
        let err_string = value.to_string();
        match value {
            // This should be unreachable because our builder checked the syntax
            // already
            regex::Error::Syntax(_) => unreachable!(),
            regex::Error::CompiledTooBig(_) => Self::RegexCompiledTooBig(err_string),
            _ => Self::RegexUnspecified(err_string),
        }
    }
}

/// Automatic conversion from
impl From<regex_syntax::Error> for Error {
    fn from(value: regex_syntax::Error) -> Self {
        Self::RegexSyntax(Box::new(value.into()))
    }
}

/// Automatic conversion from string utf8 error to our error type. If a result
/// somehow returns non-valid UTF8/UTF16, this will fire
impl From<std::str::Utf8Error> for Error {
    fn from(value: str::Utf8Error) -> Self {
        Self::Encoding(value.to_string())
    }
}

/// Serializable wrapper for a regex syntax error
///
/// Should represent both these types:
/// - <https://docs.rs/regex-syntax/latest/regex_syntax/ast/struct.Error.html>
/// - <https://docs.rs/regex-syntax/latest/regex_syntax/hir/struct.Error.html>
#[derive(Default, Debug, Serialize)]
pub struct ReSyntax {
    /// Debug representation of the syntax error type
    kind: String,
    /// Display
    message: String,
    /// Pattern that caused the error
    pattern: String,
    /// Location of the error
    span: Span,
    /// If applicable, second location of the error (e.g. for duplicates)
    auxiliary_span: Option<Span>,
}

/// Convert regex syntax errors into our common error type
impl From<regex_syntax::Error> for ReSyntax {
    fn from(value: regex_syntax::Error) -> Self {
        if let regex_syntax::Error::Parse(e) = value {
            // AST error
            Self {
                kind: format!("{:?}", e.kind()),
                message: e.kind().to_string(),
                pattern: e.pattern().to_owned(),
                span: make_span(e.pattern(), e.span()),
                auxiliary_span: e.auxiliary_span().map(|sp| make_span(e.pattern(), sp)),
            }
        } else if let regex_syntax::Error::Translate(e) = value {
            // HIR error
            Self {
                kind: format!("{:?}", e.kind()),
                message: e.kind().to_string(),
                pattern: e.pattern().to_owned(),
                span: make_span(e.pattern(), e.span()),
                auxiliary_span: None,
            }
        } else {
            Self {
                kind: "unspecified error".to_owned(),
                ..Self::default()
            }
        }
    }
}

/// Direct serializable map of `regex_syntax::ast::Span`
#[derive(Default, Debug, Serialize)]
struct Span {
    start: Position,
    end: Position,
}

/// Direct serializable map of `regex_syntax::ast::Position`
///
/// See: <https://docs.rs/regex-syntax/latest/regex_syntax/ast/struct.Position.html>
#[derive(Default, Debug, Serialize)]
struct Position {
    offset: usize,
    line: usize,
    column: usize,
}

/// Create our Span from a regex Span, converting utf8 indices to utf16
fn make_span(s: &str, span: &ReSpan) -> Span {
    let RePosition {
        offset: off8_start,
        line: line8_start,
        column: col8_start,
    } = span.start;
    let RePosition {
        offset: off8_end,
        line: line8_end,
        column: col8_end,
    } = span.end;

    let off16_start = utf16_index_bytes(s, off8_start);
    let off16_end = utf16_index_bytes(s, off8_end);

    // Need to recalculate char offset within the line
    let start_line = s.lines().nth(line8_start - 1).unwrap();
    let end_line = s.lines().nth(line8_end - 1).unwrap();

    let col16_start = utf16_index_chars(start_line, col8_start - 1) + 1;
    let col16_end = utf16_index_chars(end_line, col8_end - 1) + 1;

    Span {
        start: Position {
            offset: off16_start,
            line: line8_start,
            column: col16_start,
        },
        end: Position {
            offset: off16_end,
            line: line8_end,
            column: col16_end,
        },
    }
}
