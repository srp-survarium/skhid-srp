//! Bencoding (BEP 3), the format of a `.torrent` and of a tracker's answer.
//! Read strictly and within fixed limits: they come from strangers (a
//! tracker) or are pinned by their hash (the `.torrent`), and anything
//! that is not canonical is refused, not guessed at.

use alloc::vec::Vec;

/// The deepest nesting taken (a file tree of a torrent is a few levels).
const MAX_DEPTH: usize = 24;
/// The most values in one document.
const MAX_VALUES: usize = 200_000;

/// A dictionary's entries: key, value, the value's bytes in the document.
pub type Entries<'a> = [(&'a [u8], Value<'a>, &'a [u8])];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value<'a> {
    Int(i64),
    Bytes(&'a [u8]),
    List(Vec<Value<'a>>),
    /// The keys in their (sorted, unique) order, each with its value and
    /// the value's bytes as they are in the document.
    Dict(Vec<(&'a [u8], Value<'a>, &'a [u8])>),
}

impl<'a> Value<'a> {
    pub fn get(&self, key: &[u8]) -> Option<&Value<'a>> {
        self.entry(key).map(|(value, _)| value)
    }

    /// A key's value and its bytes in the document.
    pub fn entry(&self, key: &[u8]) -> Option<(&Value<'a>, &'a [u8])> {
        match self {
            Value::Dict(items) => items
                .iter()
                .find(|(name, _, _)| *name == key)
                .map(|(_, value, raw)| (value, *raw)),
            _ => None,
        }
    }

    pub fn bytes(&self) -> Option<&'a [u8]> {
        match self {
            Value::Bytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub fn int(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&[Value<'a>]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn dict(&self) -> Option<&Entries<'a>> {
        match self {
            Value::Dict(items) => Some(items),
            _ => None,
        }
    }
}

/// The whole of `data` as one value; anything after it is refused.
pub fn parse(data: &[u8]) -> Option<Value<'_>> {
    let mut parser = Parser {
        data,
        at: 0,
        values: 0,
    };
    let value = parser.value(0)?;
    (parser.at == data.len()).then_some(value)
}

struct Parser<'a> {
    data: &'a [u8],
    at: usize,
    values: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.data.get(self.at).copied()
    }

    /// Digits up to `end`; no sign, no leading zero but "0".
    fn digits(&mut self, end: u8) -> Option<u64> {
        let start = self.at;
        while self.peek()?.is_ascii_digit() {
            self.at += 1;
        }
        let digits = &self.data[start..self.at];
        if digits.is_empty() || digits.len() > 18 || (digits[0] == b'0' && digits.len() > 1) {
            return None;
        }
        if self.peek()? != end {
            return None;
        }
        self.at += 1;
        Some(
            digits
                .iter()
                .fold(0, |value, digit| value * 10 + u64::from(digit - b'0')),
        )
    }

    fn value(&mut self, depth: usize) -> Option<Value<'a>> {
        self.values += 1;
        if depth > MAX_DEPTH || self.values > MAX_VALUES {
            return None;
        }
        match self.peek()? {
            b'i' => {
                self.at += 1;
                let negative = self.peek()? == b'-';
                if negative {
                    self.at += 1;
                }
                let magnitude = self.digits(b'e')? as i64;
                if negative && magnitude == 0 {
                    return None;
                }
                Some(Value::Int(if negative { -magnitude } else { magnitude }))
            }
            b'l' => {
                self.at += 1;
                let mut items = Vec::new();
                while self.peek()? != b'e' {
                    items.push(self.value(depth + 1)?);
                }
                self.at += 1;
                Some(Value::List(items))
            }
            b'd' => {
                self.at += 1;
                let mut items: Vec<(&[u8], Value, &[u8])> = Vec::new();
                while self.peek()? != b'e' {
                    let key = self.string()?;
                    if items.last().is_some_and(|(last, _, _)| *last >= key) {
                        return None;
                    }
                    let start = self.at;
                    let value = self.value(depth + 1)?;
                    items.push((key, value, &self.data[start..self.at]));
                }
                self.at += 1;
                Some(Value::Dict(items))
            }
            _ => Some(Value::Bytes(self.string()?)),
        }
    }

    fn string(&mut self) -> Option<&'a [u8]> {
        let length = usize::try_from(self.digits(b':')?).ok()?;
        let end = self.at.checked_add(length)?;
        let bytes = self.data.get(self.at..end)?;
        self.at = end;
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_their_bytes() {
        let doc = b"d1:ai-3e1:bl4:spami0ee1:cd1:x0:ee";
        let value = parse(doc).unwrap();
        assert_eq!(value.get(b"a"), Some(&Value::Int(-3)));
        assert_eq!(
            value.get(b"b").unwrap().list().unwrap(),
            [Value::Bytes(b"spam"), Value::Int(0)]
        );
        let (c, raw) = value.entry(b"c").unwrap();
        assert_eq!(raw, b"d1:x0:e");
        assert_eq!(c.get(b"x"), Some(&Value::Bytes(b"")));
    }

    #[test]
    fn anything_not_canonical_is_refused() {
        for bad in [
            &b"i-0e"[..],
            b"i03e",
            b"ie",
            b"i1",
            b"03:abc",
            b"4:abc",
            b"d1:b0:1:a0:e", // unsorted
            b"d1:a0:1:a0:e", // twice
            b"di1e0:e",      // a key that is no string
            b"l",
            b"i1ei2e", // two values
            b"x",
            b"",
        ] {
            assert_eq!(parse(bad), None, "{:?}", core::str::from_utf8(bad));
        }
        let mut deep = alloc::vec![b'l'; 100];
        deep.extend(alloc::vec![b'e'; 100]);
        assert_eq!(parse(&deep), None);
    }
}
