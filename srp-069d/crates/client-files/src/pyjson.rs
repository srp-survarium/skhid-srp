//! Text output that matches Python byte for byte: `json.dumps` with its options, `repr` / `str` of numbers and lists,
//! `round(x, n)`. The server's tables use this layout, so a table written by a Python tool compares with ours byte for
//! byte.

use crate::value::Value;

/// The options of Python's `json.dumps` that the tables use.
#[derive(Debug, Clone)]
pub struct Style {
    pub indent: Option<usize>,
    pub item_sep: &'static str,
    pub key_sep: &'static str,
    pub ensure_ascii: bool,
    pub sort_keys: bool,
}

impl Default for Style {
    /// `json.dumps(x)`: no indent, `", "` and `": "`, non-ASCII escaped.
    fn default() -> Style {
        Style { indent: None, item_sep: ", ", key_sep: ": ", ensure_ascii: true, sort_keys: false }
    }
}

impl Style {
    /// `separators=(",", ":")`
    pub fn compact() -> Style {
        Style { item_sep: ",", key_sep: ":", ..Style::default() }
    }
    /// `indent=n` (Python then separates items with `","`)
    pub fn indent(n: usize) -> Style {
        Style { indent: Some(n), item_sep: ",", key_sep: ": ", ..Style::default() }
    }
    /// `ensure_ascii=False`
    pub fn utf8(mut self) -> Style {
        self.ensure_ascii = false;
        self
    }
    /// `sort_keys=True`
    pub fn sorted(mut self) -> Style {
        self.sort_keys = true;
        self
    }
}

/// `json.dumps(v, ...)`.
pub fn dumps(v: &Value, s: &Style) -> String {
    let mut out = String::new();
    write(&mut out, v, s, 0);
    out
}

fn newline(out: &mut String, s: &Style, level: usize) {
    if let Some(n) = s.indent {
        out.push('\n');
        for _ in 0..n * level {
            out.push(' ');
        }
    }
}

fn write(out: &mut String, v: &Value, s: &Style, level: usize) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&float_json(*f)),
        Value::Str(t) => string(out, t, s.ensure_ascii),
        Value::List(l) => {
            if l.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in l.iter().enumerate() {
                if i > 0 {
                    out.push_str(s.item_sep);
                }
                newline(out, s, level + 1);
                write(out, x, s, level + 1);
            }
            newline(out, s, level);
            out.push(']');
        }
        Value::Dict(d) => {
            if d.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut items: Vec<(&str, &Value)> = d.iter().collect();
            if s.sort_keys {
                items.sort_by(|a, b| a.0.cmp(b.0));
            }
            out.push('{');
            for (i, (k, x)) in items.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(s.item_sep);
                }
                newline(out, s, level + 1);
                string(out, k, s.ensure_ascii);
                out.push_str(s.key_sep);
                write(out, x, s, level + 1);
            }
            newline(out, s, level);
            out.push('}');
        }
    }
}

fn string(out: &mut String, t: &str, ascii: bool) {
    out.push('"');
    for c in t.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii && !(' '..='~').contains(&c) => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The shortest digits that read back as `f` and the decimal exponent: `f = 0.d1d2... * 10^decpt`.
fn shortest(f: f64) -> (bool, String, i32) {
    let e = format!("{:e}", f.abs());
    let (m, x) = e.split_once('e').unwrap();
    let digits: String = m.chars().filter(|c| *c != '.').collect();
    let exp: i32 = x.parse().unwrap();
    (f.is_sign_negative(), digits, exp + 1)
}

/// Python's `repr(float)`.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let (neg, digits, decpt) = shortest(f);
    let digits = if f == 0.0 { "0".to_string() } else { digits };
    let decpt = if f == 0.0 { 1 } else { decpt };
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    let n = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        s.push_str(&digits[..1]);
        if n > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        let e = decpt - 1;
        s.push('e');
        s.push(if e < 0 { '-' } else { '+' });
        s.push_str(&format!("{:02}", e.abs()));
    } else if decpt <= 0 {
        s.push_str("0.");
        for _ in 0..-decpt {
            s.push('0');
        }
        s.push_str(&digits);
    } else if decpt >= n {
        s.push_str(&digits);
        for _ in 0..decpt - n {
            s.push('0');
        }
        s.push_str(".0");
    } else {
        s.push_str(&digits[..decpt as usize]);
        s.push('.');
        s.push_str(&digits[decpt as usize..]);
    }
    s
}

/// A float as Python's json writes it.
pub fn float_json(f: f64) -> String {
    if f.is_nan() {
        "NaN".into()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else {
        float_repr(f)
    }
}

/// Python's `repr(v)` (lists as `[1, 2.5, 'a']`, `True`, `None`, dicts as `{'k': v}`).
pub fn repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => float_repr(*f),
        Value::Str(s) => str_repr(s),
        Value::List(l) => format!("[{}]", l.iter().map(repr).collect::<Vec<_>>().join(", ")),
        Value::Dict(d) => format!("{{{}}}", d.iter().map(|(k, x)| format!("{}: {}", str_repr(k), repr(x))).collect::<Vec<_>>().join(", ")),
    }
}

fn str_repr(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::new();
    o.push(q);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == q => {
                o.push('\\');
                o.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push(q);
    o
}

/// Python's `str(v)`: a string as it is, anything else as `repr`.
pub fn str(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        v => repr(v),
    }
}

/// Python's `round(f, n)`: the nearest decimal with `n` places (ties to even on the exact binary value), read back.
pub fn round(f: f64, n: usize) -> f64 {
    format!("{:.*}", n, f).parse().unwrap_or(f)
}

/// The shortest decimal (1 to 9 significant digits) that reads back as the same f32: `float("%.*g" % (n, x))` for the
/// first `n` that round-trips through f32.
pub fn f32_shortest(x: f64) -> f64 {
    for n in 1..=9 {
        let s: f64 = format!("{:.*e}", n - 1, x).parse().unwrap_or(x);
        if (s as f32) as f64 == x {
            return s;
        }
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dict, list};

    #[test]
    fn floats_print_as_python_repr() {
        for (f, s) in [
            (1.0, "1.0"),
            (0.1, "0.1"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (1e16, "1e+16"),
            (1234567890123456.0, "1234567890123456.0"),
            (0.30000001192092896, "0.30000001192092896"),
            (3.9000000953674316, "3.9000000953674316"),
            (1.5e-7, "1.5e-07"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (2.5e-5, "2.5e-05"),
            (100.0, "100.0"),
            (-12.75, "-12.75"),
        ] {
            assert_eq!(float_repr(f), s, "{f:e}");
        }
    }

    #[test]
    fn json_matches_python_layouts() {
        let v = dict! { "b" => list![1, 2.0, "x"], "a" => dict! {}, "e" => list![], "s" => "Ж\"\n" };
        assert_eq!(dumps(&v, &Style::default()), r#"{"b": [1, 2.0, "x"], "a": {}, "e": [], "s": "\u0416\"\n"}"#);
        assert_eq!(dumps(&v, &Style::compact().utf8()), "{\"b\":[1,2.0,\"x\"],\"a\":{},\"e\":[],\"s\":\"Ж\\\"\\n\"}");
        assert_eq!(dumps(&v, &Style::indent(1).sorted()), "{\n \"a\": {},\n \"b\": [\n  1,\n  2.0,\n  \"x\"\n ],\n \"e\": [],\n \"s\": \"\\u0416\\\"\\n\"\n}");
        assert_eq!(dumps(&list![dict! { "k" => 1 }], &Style::indent(0)), "[\n{\n\"k\": 1\n}\n]");
        assert_eq!(dumps(&Value::from("😀\u{7f}"), &Style::default()), "\"\\ud83d\\ude00\\u007f\"");
        assert_eq!(dumps(&Value::from("\u{7f}\u{1}"), &Style::default().utf8()), "\"\u{7f}\\u0001\"");
    }

    #[test]
    fn repr_and_str_match_python() {
        assert_eq!(repr(&list![list![1, 1], list![1, 2.5]]), "[[1, 1], [1, 2.5]]");
        assert_eq!(str(&Value::Bool(true)), "True");
        assert_eq!(str(&Value::from("a b")), "a b");
        assert_eq!(repr(&Value::from("it's")), "\"it's\"");
    }

    #[test]
    fn round_and_f32_shortest_match_python() {
        assert_eq!(round(0.30000001192092896, 6), 0.3);
        assert_eq!(round(0.0078125, 6), 0.007812); // a tie goes to even, as Python's round
        assert_eq!(round(0.0234375, 6), 0.023438);
        assert_eq!(round(2.675, 2), 2.67); // 2.675 is below the tie in binary
        assert_eq!(f32_shortest(0.1f32 as f64), 0.1);
        assert_eq!(f32_shortest(0.019999999552965164), 0.02);
        assert_eq!(f32_shortest(1.0), 1.0);
    }
}
