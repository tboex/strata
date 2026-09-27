use crate::{Error, ErrorKind, Number, Value};
use std::borrow::Cow;

pub(crate) fn parse(input: &str, max_depth: usize) -> Result<Value<'_>, Error> {
    let mut p = Parser {
        input,
        pos: 0,
        max_depth,
    };
    let value = p.value(0)?;
    p.whitespace();
    if p.pos != input.len() {
        return Err(p.error(ErrorKind::TrailingCharacters));
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
    max_depth: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }
    fn error(&self, kind: ErrorKind) -> Error {
        Error::new(self.input.as_bytes(), self.pos, kind)
    }
    fn whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, byte: u8) -> Result<(), Error> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(self.error(if self.peek().is_none() {
                ErrorKind::UnexpectedEof
            } else {
                ErrorKind::UnexpectedToken
            }))
        }
    }
    fn literal(&mut self, text: &[u8], value: Value<'a>) -> Result<Value<'a>, Error> {
        for &byte in text {
            self.expect(byte)?;
        }
        Ok(value)
    }
    fn value(&mut self, depth: usize) -> Result<Value<'a>, Error> {
        self.whitespace();
        match self.peek() {
            Some(b'n') => self.literal(b"null", Value::Null),
            Some(b't') => self.literal(b"true", Value::Bool(true)),
            Some(b'f') => self.literal(b"false", Value::Bool(false)),
            Some(b'"') => self.string().map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.number().map(Value::Number),
            Some(b'[' | b'{') if depth >= self.max_depth => Err(self.error(ErrorKind::DepthLimit)),
            Some(b'[') => self.array(depth + 1),
            Some(b'{') => self.object(depth + 1),
            None => Err(self.error(ErrorKind::UnexpectedEof)),
            _ => Err(self.error(ErrorKind::UnexpectedToken)),
        }
    }
    fn array(&mut self, depth: usize) -> Result<Value<'a>, Error> {
        self.pos += 1;
        self.whitespace();
        let mut items = Vec::new();
        if self.take(b']') {
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth)?);
            self.whitespace();
            if self.take(b']') {
                return Ok(Value::Array(items));
            }
            self.expect(b',')?;
        }
    }
    fn object(&mut self, depth: usize) -> Result<Value<'a>, Error> {
        self.pos += 1;
        self.whitespace();
        let mut entries = Vec::new();
        if self.take(b'}') {
            return Ok(Value::Object(entries));
        }
        loop {
            self.whitespace();
            let key = self.string()?;
            self.whitespace();
            self.expect(b':')?;
            entries.push((key, self.value(depth)?));
            self.whitespace();
            if self.take(b'}') {
                return Ok(Value::Object(entries));
            }
            self.expect(b',')?;
        }
    }
    fn digits(&mut self) -> Result<(), Error> {
        let start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == start {
            Err(self.error(ErrorKind::InvalidNumber))
        } else {
            Ok(())
        }
    }
    fn number(&mut self) -> Result<Number<'a>, Error> {
        let start = self.pos;
        self.take(b'-');
        if self.take(b'0') {
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(ErrorKind::InvalidNumber));
            }
        } else {
            self.digits()?;
        }
        if self.take(b'.') {
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.digits()?;
        }
        Ok(Number(&self.input[start..self.pos]))
    }
    fn hex4(&mut self) -> Result<u32, Error> {
        let mut code = 0;
        for _ in 0..4 {
            let digit = match self.peek() {
                Some(b'0'..=b'9') => self.peek().unwrap() - b'0',
                Some(b'a'..=b'f') => self.peek().unwrap() - b'a' + 10,
                Some(b'A'..=b'F') => self.peek().unwrap() - b'A' + 10,
                _ => return Err(self.error(ErrorKind::InvalidUnicode)),
            };
            self.pos += 1;
            code = code * 16 + u32::from(digit);
        }
        Ok(code)
    }
    fn unicode(&mut self) -> Result<char, Error> {
        let first = self.hex4()?;
        let code = if (0xD800..=0xDBFF).contains(&first) {
            if !self.take(b'\\') || !self.take(b'u') {
                return Err(self.error(ErrorKind::InvalidUnicode));
            }
            let second = self.hex4()?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(self.error(ErrorKind::InvalidUnicode));
            }
            0x10000 + ((first - 0xD800) << 10) + second - 0xDC00
        } else {
            first
        };
        char::from_u32(code).ok_or_else(|| self.error(ErrorKind::InvalidUnicode))
    }
    fn string(&mut self) -> Result<Cow<'a, str>, Error> {
        self.expect(b'"')?;
        let start = self.pos;
        let mut segment = start;
        let mut decoded: Option<String> = None;
        loop {
            match self.peek() {
                Some(b'"') => {
                    let end = self.pos;
                    self.pos += 1;
                    return Ok(match decoded {
                        None => Cow::Borrowed(&self.input[start..end]),
                        Some(mut s) => {
                            s.push_str(&self.input[segment..end]);
                            Cow::Owned(s)
                        }
                    });
                }
                Some(b'\\') => {
                    let out = decoded.get_or_insert_with(String::new);
                    out.push_str(&self.input[segment..self.pos]);
                    self.pos += 1;
                    let escape = self
                        .peek()
                        .ok_or_else(|| self.error(ErrorKind::UnexpectedEof))?;
                    self.pos += 1;
                    let ch = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{0008}',
                        b'f' => '\u{000c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => {
                            return Err(Error::new(
                                self.input.as_bytes(),
                                self.pos - 1,
                                ErrorKind::InvalidString,
                            ))
                        }
                    };
                    out.push(ch);
                    segment = self.pos;
                }
                Some(0..=0x1f) => return Err(self.error(ErrorKind::InvalidString)),
                Some(_) => self.pos += 1,
                None => return Err(self.error(ErrorKind::UnexpectedEof)),
            }
        }
    }
}
