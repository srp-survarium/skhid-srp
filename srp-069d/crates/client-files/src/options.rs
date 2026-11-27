//! The 0.69d binary config format (`.options`, `.profiles`, the language files `localization/<lang>/localization`, the
//! mission scripts' `.lua` tables and more).
//!
//! The file is a tree of 16-byte records, little-endian, the root record at offset 0:
//!
//! ```text
//! +0  u32  value, or the file offset of the record's data
//! +4  u16  type in the top 4 bits (the low 12 bits are not used here)
//! +6  u16  count: members of a table, bytes of a string including its NUL
//! +8  u32  file offset of the record's NUL-terminated name
//! +12 u32  not used here
//! ```
//!
//! Types: 0 bool, 1 i32, 2 f32, 3 named table, 4 list, 5 string, 6 / 7 / 8 two / three / four f32 at the data offset.
//! A table's members are `count` records starting at its data offset. Names and most strings are Windows-1251; the
//! language files keep UTF-8 strings.

use std::collections::HashMap;

use crate::value::{Dict, Value};
use crate::{cp1251, Error, Result};

/// How the strings of a file are encoded (names are always Windows-1251).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Cp1251,
    Utf8,
}

const MAX_DEPTH: u32 = 64;

struct Reader<'a> {
    b: &'a [u8],
    enc: Encoding,
}

impl Reader<'_> {
    fn err(&self, at: usize, why: impl Into<String>) -> Error {
        Error::format(format!("config record at {at:#x}"), why)
    }

    fn u32_at(&self, o: usize) -> Result<u32> {
        self.b.get(o..o + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| self.err(o, "outside the file"))
    }

    fn cstr(&self, o: usize) -> Result<&[u8]> {
        let s = self.b.get(o..).ok_or_else(|| self.err(o, "name outside the file"))?;
        let n = s.iter().position(|&c| c == 0).ok_or_else(|| self.err(o, "name not terminated"))?;
        Ok(&s[..n])
    }

    fn name(&self, o: usize) -> Result<String> {
        let off = self.u32_at(o + 8)? as usize;
        cp1251::decode(self.cstr(off)?).ok_or_else(|| self.err(o, "name is not Windows-1251"))
    }

    fn text(&self, o: usize, raw: &[u8]) -> Result<String> {
        match self.enc {
            Encoding::Cp1251 => cp1251::decode(raw).ok_or_else(|| self.err(o, "string is not Windows-1251")),
            Encoding::Utf8 => String::from_utf8(raw.to_vec()).map_err(|_| self.err(o, "string is not UTF-8")),
        }
    }

    fn f32_at(&self, o: usize) -> Result<f64> {
        Ok(f32::from_bits(self.u32_at(o)?) as f64)
    }

    fn record(&self, o: usize, depth: u32) -> Result<Value> {
        if depth > MAX_DEPTH {
            return Err(self.err(o, "tables nested too deep"));
        }
        let data = self.u32_at(o)?;
        let tc = self.b.get(o + 4..o + 6).map(|s| u16::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| self.err(o, "outside the file"))?;
        let cnt = self.b.get(o + 6..o + 8).map(|s| u16::from_le_bytes(s.try_into().unwrap())).ok_or_else(|| self.err(o, "outside the file"))? as usize;
        let d = data as usize;
        Ok(match tc >> 12 {
            0 => Value::Bool(data != 0),
            1 => Value::Int(data as i32 as i64),
            2 => Value::Float(f32::from_bits(data) as f64),
            3 => {
                let mut t = Dict::new();
                let mut seen: HashMap<String, usize> = HashMap::new();
                for i in 0..cnt {
                    let ko = d + 16 * i;
                    let k = self.name(ko)?;
                    let v = self.record(ko, depth + 1)?;
                    // a repeated name keeps its first place and its last value
                    match seen.get(&k) {
                        Some(&at) => t.replace_at(at, v),
                        None => {
                            seen.insert(k.clone(), t.len());
                            t.push_new(k, v);
                        }
                    }
                }
                Value::Dict(t)
            }
            4 => Value::List((0..cnt).map(|i| self.record(d + 16 * i, depth + 1)).collect::<Result<_>>()?),
            5 => {
                let raw = self.b.get(d..d + cnt.saturating_sub(1)).ok_or_else(|| self.err(o, "string outside the file"))?;
                Value::Str(self.text(o, raw)?)
            }
            t @ 6..=8 => Value::List((0..(t as usize - 4)).map(|i| self.f32_at(d + 4 * i).map(Value::Float)).collect::<Result<_>>()?),
            t => return Err(self.err(o, format!("unknown record type {t}"))),
        })
    }
}

/// Decode a whole config file.
pub fn decode(bytes: &[u8], enc: Encoding) -> Result<Value> {
    Reader { b: bytes, enc }.record(0, 0)
}

/// Encode a value in the same format (tests and fixtures). Floats are stored as f32, ints as i32; `null` is not
/// representable and becomes `false`.
pub fn encode(v: &Value, enc: Encoding) -> Vec<u8> {
    // layout: records first (breadth by table), then names and strings appended to a heap
    struct W {
        recs: Vec<u8>,
        heap: Vec<u8>,
        enc: Encoding,
    }
    fn enc_text(s: &str, enc: Encoding) -> Vec<u8> {
        match enc {
            Encoding::Utf8 => s.as_bytes().to_vec(),
            Encoding::Cp1251 => s
                .chars()
                .map(|c| {
                    if (c as u32) < 0x80 {
                        return c as u8;
                    }
                    (0x80u8..=0xFF).find(|&b| cp1251::decode(&[b]).and_then(|t| t.chars().next()) == Some(c)).unwrap_or(b'?')
                })
                .collect(),
        }
    }
    impl W {
        fn heap_str(&mut self, b: &[u8]) -> u32 {
            let at = self.heap.len() as u32;
            self.heap.extend_from_slice(b);
            self.heap.push(0);
            at | 0x8000_0000 // marked: heap offsets are fixed up at the end
        }
        fn put(&mut self, at: usize, name: &str, v: &Value) {
            let name_off = self.heap_str(&enc_text(name, Encoding::Cp1251));
            let (data, ty, cnt): (u32, u16, u16) = match v {
                Value::Null => (0, 0, 0),
                Value::Bool(b) => (*b as u32, 0, 0),
                Value::Int(i) => (*i as i32 as u32, 1, 0),
                Value::Float(f) => ((*f as f32).to_bits(), 2, 0),
                Value::Str(s) => {
                    let b = enc_text(s, self.enc);
                    let n = b.len() as u16 + 1;
                    (self.heap_str(&b), 5, n)
                }
                Value::List(l) if !l.is_empty() && l.len() <= 4 && l.len() >= 2 && l.iter().all(|x| matches!(x, Value::Float(_))) => {
                    let mut b = Vec::new();
                    for x in l {
                        b.extend_from_slice(&(x.as_f64().unwrap() as f32).to_bits().to_le_bytes());
                    }
                    let at = self.heap.len() as u32;
                    self.heap.extend_from_slice(&b);
                    (at | 0x8000_0000, 4 + l.len() as u16, 0)
                }
                Value::List(l) => {
                    let start = self.recs.len();
                    self.recs.resize(start + 16 * l.len(), 0);
                    for (i, x) in l.iter().enumerate() {
                        self.put(start + 16 * i, "", x);
                    }
                    (start as u32, 4, l.len() as u16)
                }
                Value::Dict(d) => {
                    let start = self.recs.len();
                    self.recs.resize(start + 16 * d.len(), 0);
                    for (i, (k, x)) in d.iter().enumerate() {
                        self.put(start + 16 * i, k, x);
                    }
                    (start as u32, 3, d.len() as u16)
                }
            };
            self.recs[at..at + 4].copy_from_slice(&data.to_le_bytes());
            self.recs[at + 4..at + 6].copy_from_slice(&(ty << 12).to_le_bytes());
            self.recs[at + 6..at + 8].copy_from_slice(&cnt.to_le_bytes());
            self.recs[at + 8..at + 12].copy_from_slice(&name_off.to_le_bytes());
        }
    }
    let mut w = W { recs: vec![0u8; 16], heap: Vec::new(), enc };
    w.put(0, "", v);
    // fix up heap offsets (marked with the top bit) now that the record area's size is known
    let base = w.recs.len() as u32;
    let n = w.recs.len() / 16;
    for i in 0..n {
        let r = 16 * i;
        let ty = u16::from_le_bytes(w.recs[r + 4..r + 6].try_into().unwrap()) >> 12;
        for (field, is_heap) in [(r, matches!(ty, 5..=8)), (r + 8, true)] {
            let x = u32::from_le_bytes(w.recs[field..field + 4].try_into().unwrap());
            if is_heap && x & 0x8000_0000 != 0 {
                w.recs[field..field + 4].copy_from_slice(&((x & 0x7FFF_FFFF) + base).to_le_bytes());
            }
        }
    }
    let mut out = w.recs;
    out.extend_from_slice(&w.heap);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dict, list};

    #[test]
    fn every_record_type_round_trips() {
        let v = dict! {
            "flag" => true,
            "n" => -7,
            "f" => 0.25,
            "name" => "ак_74",
            "list" => list![1, 2, dict! { "x" => 3 }],
            "v3" => list![1.0, 2.5, -3.0],
            "empty" => dict! {},
        };
        let b = encode(&v, Encoding::Cp1251);
        assert_eq!(decode(&b, Encoding::Cp1251).unwrap(), v);
        let u = dict! { "strings" => dict! { "st_a" => "Привіт", "st_b" => "中文" } };
        assert_eq!(decode(&encode(&u, Encoding::Utf8), Encoding::Utf8).unwrap(), u);
    }

    #[test]
    fn floats_are_f32_widened() {
        let b = encode(&dict! { "p" => 0.1 }, Encoding::Cp1251);
        assert_eq!(decode(&b, Encoding::Cp1251).unwrap()["p"], Value::Float(0.1f32 as f64));
    }

    #[test]
    fn a_repeated_name_keeps_its_first_place_and_its_last_value() {
        let mut b = encode(&dict! { "a" => 1, "b" => 2, "c" => 3 }, Encoding::Cp1251);
        // rename "c" to "a": member 2 (record at 16 + 32) takes member 0's name offset (record at 16)
        let name0 = b[16 + 8..16 + 12].to_vec();
        b[48 + 8..48 + 12].copy_from_slice(&name0);
        let v = decode(&b, Encoding::Cp1251).unwrap();
        assert_eq!(v.as_dict().unwrap().keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(v["a"], Value::Int(3));
    }

    #[test]
    fn damaged_files_are_errors() {
        assert!(decode(&[1, 2, 3], Encoding::Cp1251).is_err());
        let mut b = encode(&dict! { "a" => "x" }, Encoding::Cp1251);
        b[4..6].copy_from_slice(&(0xF000u16).to_le_bytes());
        assert!(decode(&b, Encoding::Cp1251).is_err());
        // a table that contains itself
        let mut l = vec![0u8; 16];
        l[4..6].copy_from_slice(&(0x4000u16).to_le_bytes());
        l[6..8].copy_from_slice(&1u16.to_le_bytes());
        assert!(decode(&l, Encoding::Cp1251).is_err());
    }
}
