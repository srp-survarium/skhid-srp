//! An ordered, JSON-like value: what a binary config decodes to and what the extractors build their tables from.
//! Tables keep their keys in file order (output order matters for byte-identical tables).

use std::ops::Index;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Value>),
    Dict(Dict),
}

/// Keys in insertion order. Setting an existing key replaces its value and keeps its place.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dict(Vec<(String, Value)>);

static NULL: Value = Value::Null;

impl Dict {
    pub fn new() -> Dict {
        Dict(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, k: &str) -> Option<&Value> {
        self.0.iter().find(|(n, _)| n == k).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, k: &str) -> Option<&mut Value> {
        self.0.iter_mut().find(|(n, _)| n == k).map(|(_, v)| v)
    }

    pub fn contains(&self, k: &str) -> bool {
        self.get(k).is_some()
    }

    pub fn set(&mut self, k: impl Into<String>, v: impl Into<Value>) {
        let k = k.into();
        let v = v.into();
        match self.0.iter_mut().find(|(n, _)| *n == k) {
            Some(slot) => slot.1 = v,
            None => self.0.push((k, v)),
        }
    }

    /// Append without looking for the key (the caller knows it is new).
    pub(crate) fn push_new(&mut self, k: String, v: Value) {
        self.0.push((k, v));
    }

    /// Replace the value at position `i`.
    pub(crate) fn replace_at(&mut self, i: usize, v: Value) {
        self.0[i].1 = v;
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(k, _)| k.as_str())
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.iter().map(|(_, v)| v)
    }

    /// Sort the keys (by code point, as Python's `sorted` does for strings).
    pub fn sort_keys(&mut self) {
        self.0.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// Build from pairs; a repeated key keeps its first place and its last value.
    pub fn from_pairs<K: Into<String>, V: Into<Value>>(pairs: impl IntoIterator<Item = (K, V)>) -> Dict {
        let mut d = Dict::new();
        for (k, v) in pairs {
            d.set(k, v);
        }
        d
    }
}

impl Value {
    pub fn get(&self, k: &str) -> Option<&Value> {
        match self {
            Value::Dict(d) => d.get(k),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_dict(&self) -> Option<&Dict> {
        match self {
            Value::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// An integer (a bool counts as 0 / 1, as in Python).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Bool(b) => Some(*b as i64),
            _ => None,
        }
    }

    /// A number as f64 (ints and bools convert).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            Value::Bool(b) => Some(*b as i64 as f64),
            _ => None,
        }
    }

    /// The members of a list, or nothing (a table that is empty in the file decodes as an empty named table).
    pub fn items(&self) -> &[Value] {
        self.as_list().unwrap_or(&[])
    }

    /// The entries of a named table, or nothing.
    pub fn entries(&self) -> Box<dyn Iterator<Item = (&str, &Value)> + '_> {
        match self {
            Value::Dict(d) => Box::new(d.iter()),
            _ => Box::new(std::iter::empty()),
        }
    }

    /// Python truthiness: null, false, 0, 0.0, "" and empty tables are false.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Dict(d) => !d.is_empty(),
        }
    }
}

impl Index<&str> for Value {
    type Output = Value;
    /// The member `k` of a named table, or null.
    fn index(&self, k: &str) -> &Value {
        self.get(k).unwrap_or(&NULL)
    }
}

impl Index<usize> for Value {
    type Output = Value;
    /// The `i`th member of a list, or null.
    fn index(&self, i: usize) -> &Value {
        self.as_list().and_then(|l| l.get(i)).unwrap_or(&NULL)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<i64> for Value {
    fn from(i: i64) -> Value {
        Value::Int(i)
    }
}
impl From<i32> for Value {
    fn from(i: i32) -> Value {
        Value::Int(i as i64)
    }
}
impl From<u32> for Value {
    fn from(i: u32) -> Value {
        Value::Int(i as i64)
    }
}
impl From<usize> for Value {
    fn from(i: usize) -> Value {
        Value::Int(i as i64)
    }
}
impl From<f64> for Value {
    fn from(f: f64) -> Value {
        Value::Float(f)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}
impl From<Dict> for Value {
    fn from(d: Dict) -> Value {
        Value::Dict(d)
    }
}
impl From<&Value> for Value {
    fn from(v: &Value) -> Value {
        v.clone()
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Value {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

/// `dict! { "a" => 1, "b" => "x" }`: an ordered [`Dict`] (as a [`Value`]).
#[macro_export]
macro_rules! dict {
    () => { $crate::Value::Dict($crate::Dict::new()) };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut d = $crate::Dict::new();
        $( d.set($k, $v); )+
        $crate::Value::Dict(d)
    }};
}

/// `list![a, b]`: a list [`Value`] of mixed members.
#[macro_export]
macro_rules! list {
    ($($v:expr),* $(,)?) => { $crate::Value::List(vec![$($crate::Value::from($v)),*]) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_key_keeps_its_place_and_takes_the_new_value() {
        let d = Dict::from_pairs([("a", 1), ("b", 2), ("a", 3)]);
        assert_eq!(d.keys().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(d.get("a"), Some(&Value::Int(3)));
    }

    #[test]
    fn indexing_a_missing_member_is_null() {
        let v = dict! { "a" => list![1, "x"] };
        assert_eq!(v["a"][1].as_str(), Some("x"));
        assert!(v["b"]["c"][7].is_null());
        assert!(!dict! {}.truthy());
        assert!(Value::Float(0.5).truthy());
    }
}
