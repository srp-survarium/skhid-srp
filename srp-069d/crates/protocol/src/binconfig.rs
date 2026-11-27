//! The 0.69d binary config format (the `.options` files; also the `BC` store tab blob, reader 0x200196540 ->
//! 0x2000CC680). Layout (PROVEN from game files and the lookup 0x2000CC0E0):
//!
//! * 16-byte records `u32 data | u16 type << 12 | u16 count | u32 name offset | u32 hash`; the root record sits at offset 0
//!   and its `data` points at its children (right after it in the files);
//! * types: 0 bool, 1 int (`data` = value), 2 float (`data` = bits), 3 named table, 4 indexed table, 5 string (`data` =
//!   offset of the NUL-terminated cp1251 bytes, `count` = length + 1);
//! * a named table's children carry the name (offset of a NUL-terminated string) and the CRC-16 of it in the HIGH half of
//!   the `hash` field (the low half is 0); the children are SORTED by that CRC (the lookup is a binary search over the
//!   16-bit hash at record offset 0xE). The CRC is CRC-16/ARC (init 0, reflected 0xA001), 0x2000CC15B..0x2000CC196.
//!
//! The writer is used for the store tab blob; the reader exists for tests.

use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Float(f32),
    Str(String),
    /// named table (children are written sorted by the CRC of the name)
    Table(Vec<(String, Value)>),
    /// indexed table
    List(Vec<Value>),
}

/// CRC-16/ARC of a name, the key the client's lookup compares.
pub fn crc16(name: &str) -> u16 {
    let mut c: u16 = 0;
    for &b in name.as_bytes() {
        c ^= u16::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xA001 } else { c >> 1 };
        }
    }
    c
}

struct Fix {
    at: usize,
    pool: usize,
}

struct Enc {
    main: Vec<u8>,
    pool: Vec<u8>,
    data_fix: Vec<Fix>,
    name_fix: Vec<Fix>,
}

impl Enc {
    fn pool_str(&mut self, s: &str) -> usize {
        let o = self.pool.len();
        self.pool.extend_from_slice(s.as_bytes());
        self.pool.push(0);
        o
    }

    /// Write the record of `v` at `pos` (already reserved), its name and its children.
    fn record(&mut self, pos: usize, name: Option<&str>, v: &Value) {
        let (ty, data, count): (u16, u32, u16) = match v {
            Value::Bool(b) => (0, u32::from(*b), 0),
            Value::Int(i) => (1, *i as u32, 0),
            Value::Float(f) => (2, f.to_bits(), 0),
            Value::Str(s) => {
                let o = self.pool_str(s);
                self.data_fix.push(Fix { at: pos, pool: o });
                (5, 0, (s.len() + 1).min(0xFFF) as u16)
            }
            Value::Table(items) => {
                let mut sorted: Vec<&(String, Value)> = items.iter().collect();
                sorted.sort_by_key(|(k, _)| crc16(k));
                let base = self.reserve(sorted.len());
                for (i, (k, c)) in sorted.iter().enumerate() {
                    self.record(base + 16 * i, Some(k), c);
                }
                (3, base as u32, sorted.len() as u16)
            }
            Value::List(items) => {
                let base = self.reserve(items.len());
                for (i, c) in items.iter().enumerate() {
                    self.record(base + 16 * i, None, c);
                }
                (4, base as u32, items.len() as u16)
            }
        };
        self.main[pos..pos + 4].copy_from_slice(&data.to_le_bytes());
        self.main[pos + 4..pos + 6].copy_from_slice(&(ty << 12).to_le_bytes());
        self.main[pos + 6..pos + 8].copy_from_slice(&count.to_le_bytes());
        if let Some(n) = name {
            let o = self.pool_str(n);
            self.name_fix.push(Fix { at: pos + 8, pool: o });
            self.main[pos + 14..pos + 16].copy_from_slice(&crc16(n).to_le_bytes());
        }
    }

    fn reserve(&mut self, n: usize) -> usize {
        let base = self.main.len();
        self.main.resize(base + 16 * n, 0);
        base
    }
}

/// Encode a named root table.
pub fn encode(root: &[(String, Value)]) -> Vec<u8> {
    encode_value(&Value::Table(root.to_vec()))
}

/// Encode any root value (the `94` record bodies have an indexed table as their root, e.g. the event window's `ui` list).
pub fn encode_value(root: &Value) -> Vec<u8> {
    let mut e = Enc { main: vec![0; 16], pool: Vec::new(), data_fix: Vec::new(), name_fix: Vec::new() };
    e.record(0, None, root);
    let base = e.main.len();
    let mut out = std::mem::take(&mut e.main);
    for f in &e.data_fix {
        out[f.at..f.at + 4].copy_from_slice(&((base + f.pool) as u32).to_le_bytes());
    }
    for f in &e.name_fix {
        out[f.at..f.at + 4].copy_from_slice(&((base + f.pool) as u32).to_le_bytes());
    }
    out.extend_from_slice(&e.pool);
    out
}

/// Decode a file (tests): the root as a named table.
pub fn decode(b: &[u8]) -> Result<Vec<(String, Value)>, Error> {
    match build(b, 0, 0)? {
        Value::Table(t) => Ok(t),
        _ => Err(Error::Invalid("binary config root is not a table")),
    }
}

fn cstr(b: &[u8], o: usize) -> Result<String, Error> {
    let tail = b.get(o..).ok_or(Error::Invalid("binary config string offset"))?;
    let e = tail.iter().position(|c| *c == 0).ok_or(Error::Invalid("binary config string end"))?;
    Ok(String::from_utf8_lossy(&tail[..e]).into_owned())
}

fn build(b: &[u8], pos: usize, depth: u32) -> Result<Value, Error> {
    if depth > 16 {
        return Err(Error::Invalid("binary config too deep"));
    }
    let rec = b.get(pos..pos + 16).ok_or(Error::Invalid("binary config record"))?;
    let data = u32::from_le_bytes(rec[0..4].try_into().unwrap());
    let tc = u16::from_le_bytes(rec[4..6].try_into().unwrap());
    let count = u16::from_le_bytes(rec[6..8].try_into().unwrap()) as usize;
    Ok(match tc >> 12 {
        0 => Value::Bool(data != 0),
        1 => Value::Int(data as i32),
        2 => Value::Float(f32::from_bits(data)),
        3 => {
            let mut v = Vec::new();
            for i in 0..count {
                let kp = data as usize + 16 * i;
                let k = b.get(kp..kp + 16).ok_or(Error::Invalid("binary config child"))?;
                let name = cstr(b, u32::from_le_bytes(k[8..12].try_into().unwrap()) as usize)?;
                v.push((name, build(b, kp, depth + 1)?));
            }
            Value::Table(v)
        }
        4 => Value::List((0..count).map(|i| build(b, data as usize + 16 * i, depth + 1)).collect::<Result<_, _>>()?),
        5 => Value::Str(cstr(b, data as usize)?),
        _ => return Err(Error::Invalid("binary config type")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_game_files() {
        // hashes read from gameplay/event_rules/hero_of_the_day.options
        assert_eq!(crc16("parameters"), 0x9d7b);
        assert_eq!(crc16("competitive_groups"), 0x8434);
        assert_eq!(crc16("min_level"), 0xdbf1);
        assert_eq!(crc16("max_level"), 0xbc87);
        assert_eq!(crc16("minutes_operating_period"), 0xe236);
    }

    #[test]
    fn an_indexed_root_round_trips() {
        let root = Value::List(vec![Value::Table(vec![
            ("type".into(), Value::Str("TextComponent".into())),
            ("objectData".into(), Value::Table(vec![("label".into(), Value::Str("A".into()))])),
        ])]);
        let b = encode_value(&root);
        assert_eq!(b[5] >> 4, 4, "the root record is an indexed table");
        let Value::List(items) = build(&b, 0, 0).unwrap() else { panic!("not a list") };
        assert_eq!(items.len(), 1);
        let Value::Table(t) = &items[0] else { panic!("not a table") };
        assert!(t.iter().any(|(k, v)| k == "type" && *v == Value::Str("TextComponent".into())));
    }

    #[test]
    fn round_trip_and_sorted_children() {
        let root = vec![
            ("shop_items".to_string(), Value::List(vec![Value::Table(vec![("id".into(), Value::Int(7)), ("label".into(), Value::Str("Hi".into()))])])),
            ("currency_type".to_string(), Value::Int(1)),
            ("currency_sign".to_string(), Value::Str("G".into())),
            ("flag".to_string(), Value::Bool(true)),
            ("f".to_string(), Value::Float(0.5)),
        ];
        let b = encode(&root);
        // the root record points right behind itself
        assert_eq!(u32::from_le_bytes(b[0..4].try_into().unwrap()), 16);
        assert_eq!(b[5] >> 4, 3);
        let back = decode(&b).unwrap();
        // every table's children come back sorted by key hash (the root and the nested ones alike)
        fn sorted(t: &[(String, Value)]) -> Vec<(String, Value)> {
            let mut v: Vec<(String, Value)> = t.iter().map(|(k, x)| (k.clone(), sorted_value(x))).collect();
            v.sort_by_key(|(k, _)| crc16(k));
            v
        }
        fn sorted_value(x: &Value) -> Value {
            match x {
                Value::Table(t) => Value::Table(sorted(t)),
                Value::List(l) => Value::List(l.iter().map(sorted_value).collect()),
                other => other.clone(),
            }
        }
        assert_eq!(back, sorted(&root));
        // hashes ascend
        let hs: Vec<u16> = (0..5).map(|i| u16::from_le_bytes(b[16 + 16 * i + 14..16 + 16 * i + 16].try_into().unwrap())).collect();
        assert!(hs.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn empty_lists_and_tables() {
        let b = encode(&[("shop_items".to_string(), Value::List(vec![]))]);
        assert_eq!(decode(&b).unwrap(), vec![("shop_items".to_string(), Value::List(vec![]))]);
        assert_eq!(decode(&encode(&[])).unwrap(), vec![]);
    }
}
