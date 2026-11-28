//! The command line as this process reads it. fork() copies argv, so a zygote forked from the base process
//! (`daemon::base_loop`) would read the base's arguments; it sets its own list once, right after the fork and before
//! anything reads it ([`set_override`]). Every reader goes through here; only `run::main` parses the real argv itself.
//! A match child forked from such a zygote inherits the zygote's list.

use std::sync::OnceLock;

static OVERRIDE: OnceLock<Vec<String>> = OnceLock::new();

/// The arguments (argv[0] first): the override when set, else the real ones.
pub fn args() -> Vec<String> {
    match OVERRIDE.get() {
        Some(v) => v.clone(),
        None => std::env::args().collect(),
    }
}

/// Set this process's list (once; false when it was set already).
pub fn set_override(v: Vec<String>) -> bool {
    OVERRIDE.set(v).is_ok()
}

/// `k` is one of the arguments.
pub fn has(k: &str) -> bool {
    has_in(&args(), k)
}

/// The argument after the first `k`.
pub fn value(k: &str) -> Option<String> {
    value_in(&args(), k)
}

pub fn has_in(a: &[String], k: &str) -> bool {
    a.iter().any(|x| x == k)
}

pub fn value_in(a: &[String], k: &str) -> Option<String> {
    a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
}

/// Replace the argument after the first `k` (append `k v` when `k` is missing or last).
pub fn set_value(a: &mut Vec<String>, k: &str, v: &str) {
    match a.iter().position(|x| x == k) {
        Some(i) if i + 1 < a.len() => a[i + 1] = v.to_string(),
        Some(_) => a.push(v.to_string()),
        None => {
            a.push(k.to_string());
            a.push(v.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn a_value_is_the_argument_after_the_first_key() {
        let a = v(&["wh", "--level", "pve_01/v1_maze2", "--out", "/o", "--level", "x"]);
        assert_eq!(value_in(&a, "--level").as_deref(), Some("pve_01/v1_maze2"));
        assert_eq!(value_in(&a, "--out").as_deref(), Some("/o"));
        assert_eq!(value_in(&a, "--prewarm"), None);
        assert_eq!(value_in(&v(&["wh", "--out"]), "--out"), None, "a key at the end has no value");
    }

    #[test]
    fn has_matches_whole_arguments_only() {
        let a = v(&["wh", "--null-render", "--tick-frames"]);
        assert!(has_in(&a, "--null-render"));
        assert!(!has_in(&a, "--null"));
        assert!(!has_in(&a, "--render-world"));
    }

    #[test]
    fn set_value_replaces_or_appends() {
        let mut a = v(&["wh", "--zygote-fd", "3", "--level", "l"]);
        set_value(&mut a, "--zygote-fd", "17");
        assert_eq!(a, v(&["wh", "--zygote-fd", "17", "--level", "l"]));
        set_value(&mut a, "--out", "/z");
        assert_eq!(value_in(&a, "--out").as_deref(), Some("/z"));
        let mut b = v(&["wh", "--out"]);
        set_value(&mut b, "--out", "/y");
        assert_eq!(b, v(&["wh", "--out", "/y"]));
    }
}
