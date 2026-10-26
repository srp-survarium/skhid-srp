//! The keys a tuning file or a bench patch used before their field moved into a section of its feature. One
//! table, applied to the JSON before it is read: serde's `alias` renames a key, not a path, and a table also
//! serves the patch of `SRP_BENCH_TUNING` and the old files in the traces. A moved field adds a row.

use serde_json::Value;

/// `(old pointer, new pointer)`, RFC 6901. A value keeps its form (the new field reads what the old one did).
pub(super) const MOVED: &[(&str, &str)] = &[
    ("/hazard_margin", "/hazard/margin_m"),
    ("/escape_ceiling", "/hazard/escape"),
    ("/floor_paths", "/grid/floor_paths"),
    ("/gap_links", "/grid/gap_links"),
];

/// `value` with every old key of [`MOVED`] at its new place. Both the old and the new key given is refused.
pub(super) fn upgrade(value: &mut Value) -> Result<(), String> {
    for &(old, new) in MOVED {
        let Some(at) = old.rfind('/') else { continue };
        let Some(parent) = value.pointer_mut(&old[..at]).and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(moved) = parent.remove(&old[at + 1..]) else { continue };
        if value.pointer(new).is_some() {
            return Err(format!("{old} and {new} both given: {old} is {new} now"));
        }
        put(value, new, moved);
    }
    Ok(())
}

/// `moved` at `pointer`, making the objects on the way.
fn put(value: &mut Value, pointer: &str, moved: Value) {
    let mut at = value;
    let mut names = pointer.split('/').skip(1).peekable();
    while let Some(name) = names.next() {
        if !at.is_object() {
            *at = Value::Object(Default::default());
        }
        let map = at.as_object_mut().expect("just made an object");
        if names.peek().is_none() {
            map.insert(name.to_owned(), moved);
            return;
        }
        at = map.entry(name.to_owned()).or_insert_with(|| Value::Object(Default::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tuning::Tuning;

    fn tuning_of(text: &str) -> Result<Tuning, String> {
        Tuning::from_value(serde_json::from_str(text).unwrap())
    }

    /// Every old key still reads as the field it moved to, with the value it had.
    #[test]
    fn every_old_key_reads_as_its_new_one() {
        let defaults = Tuning::default();
        for &(old, new) in MOVED {
            // The default's own value under the old key is the default.
            let default = serde_json::to_value(&defaults).unwrap().pointer(new).cloned().expect("the new key exists");
            let key = old.trim_start_matches('/');
            let mut old_form = serde_json::Map::new();
            old_form.insert(key.to_owned(), default.clone());
            let old_form = Value::Object(old_form);
            let new_form = {
                let mut v = Value::Object(Default::default());
                put(&mut v, new, default);
                v
            };
            assert_eq!(Tuning::from_value(old_form).unwrap(), Tuning::from_value(new_form).unwrap(), "{old}");
        }
        // And a changed value lands where the new key puts it.
        let t = tuning_of(r#"{"hazard_margin": 2.5, "escape_ceiling": false}"#).unwrap();
        assert_eq!(t.hazard.margin_m.get(), 2.5);
        assert_eq!(t.hazard.escape, super::super::EscapeRule::Nearest);
        // An old flag and a new name are the same rule.
        assert_eq!(tuning_of(r#"{"hazard":{"escape":"nearest"}}"#).unwrap(), t_with_nearest());
        assert_eq!(tuning_of(r#"{"escape_ceiling": true}"#).unwrap(), defaults);
    }

    fn t_with_nearest() -> Tuning {
        let mut t = Tuning::default();
        t.hazard.escape = super::super::EscapeRule::Nearest;
        t
    }

    #[test]
    fn the_old_and_the_new_key_together_are_refused() {
        assert!(tuning_of(r#"{"hazard_margin": 2, "hazard": {"margin_m": 3}}"#).is_err());
        assert!(tuning_of(r#"{"escape_ceiling": "yes"}"#).is_err());
        assert!(tuning_of(r#"{"hazard_margin": -1}"#).is_err());
    }
}
