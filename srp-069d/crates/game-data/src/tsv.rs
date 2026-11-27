//! Tab-separated tables, read like Python's `csv.DictReader(delimiter="\t")`:
//! a field that starts with `"` is quoted (`""` is a quote, tabs and newlines may sit inside);
//! a quote anywhere else is a plain character.

use std::collections::HashMap;

/// One table: header names and rows. Missing trailing cells read as empty strings.
pub struct Table {
    index: HashMap<String, usize>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn parse(text: &str) -> Table {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut records: Vec<Vec<String>> = Vec::new();
        let mut rec: Vec<String> = Vec::new();
        let mut field = String::new();
        let mut chars = text.chars().peekable();
        let mut at_start = true; // at the first char of a field
        let mut quoted = false;
        while let Some(c) = chars.next() {
            if quoted {
                if c == '"' {
                    if chars.peek() == Some(&'"') {
                        field.push('"');
                        chars.next();
                    } else {
                        quoted = false;
                    }
                } else {
                    field.push(c);
                }
                continue;
            }
            match c {
                '"' if at_start => {
                    quoted = true;
                    at_start = false;
                }
                '\t' => {
                    rec.push(std::mem::take(&mut field));
                    at_start = true;
                }
                '\r' if chars.peek() == Some(&'\n') => {}
                '\n' => {
                    rec.push(std::mem::take(&mut field));
                    records.push(std::mem::take(&mut rec));
                    at_start = true;
                }
                _ => {
                    field.push(c);
                    at_start = false;
                }
            }
        }
        if !field.is_empty() || !rec.is_empty() {
            rec.push(field);
            records.push(rec);
        }
        // Python's csv yields [] for an empty line and DictReader skips it
        records.retain(|r| !(r.is_empty() || (r.len() == 1 && r[0].is_empty())));
        let mut it = records.into_iter();
        let header = it.next().unwrap_or_default();
        let index = header.into_iter().enumerate().map(|(i, h)| (h, i)).collect();
        Table { index, rows: it.collect() }
    }

    /// Cell of `row` in column `name`; "" when the column or the cell does not exist.
    pub fn get<'a>(&self, row: &'a [String], name: &str) -> &'a str {
        self.index.get(name).and_then(|&i| row.get(i)).map(|s| s.as_str()).unwrap_or("")
    }
}

/// Python `int(float(s))` for a non-empty numeric string (truncates towards zero).
pub fn int_of_float(s: &str) -> Option<i64> {
    s.trim().parse::<f64>().ok().map(|f| f as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_and_short_rows() {
        let t = Table::parse("a\tb\tc\n1\t\"x \"\"q\"\"\ty\"\tz\n2\tm\"n\n\n");
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.get(&t.rows[0], "b"), "x \"q\"\ty");
        assert_eq!(t.get(&t.rows[0], "c"), "z");
        assert_eq!(t.get(&t.rows[1], "b"), "m\"n");
        assert_eq!(t.get(&t.rows[1], "c"), "");
        assert_eq!(t.get(&t.rows[1], "nope"), "");
    }
}
