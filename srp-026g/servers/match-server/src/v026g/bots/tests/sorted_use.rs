//! The `use` lines of the two files every feature adds to (`bots/mod.rs`, `tuning/mod.rs`) are one item a line and
//! sorted, so that two PRs that each add a type touch two lines and not the same one (`pub use team::{A, B}` made
//! #588 and #565 conflict). rustfmt keeps them as they are (`imports_granularity` is `Preserve`, there is no
//! `rustfmt.toml`) and orders a run its own way: `crate::` first, then by the path.
//!
//! A run is the `use` lines at the start of a line with nothing else between them (an attribute does not end it).

use std::path::Path;

/// The lines of `source` that break the rule: a `use` with a brace list, or one out of order in its run, as
/// `(line number, what)`.
fn broken(source: &str) -> Vec<(usize, String)> {
    let mut wrong = Vec::new();
    let mut previous: Option<(bool, String)> = None;
    for (n, line) in source.lines().enumerate() {
        let line = line.trim_end();
        let path = line
            .strip_prefix("pub(crate) ")
            .or_else(|| line.strip_prefix("pub(super) "))
            .or_else(|| line.strip_prefix("pub "))
            .unwrap_or(line)
            .strip_prefix("use ")
            .and_then(|rest| rest.strip_suffix(';'));
        match path {
            Some(path) => {
                if path.contains('{') {
                    wrong.push((n + 1, format!("a brace list: {path}")));
                }
                // rustfmt's order: `crate::` first, then by the path.
                let key = (!path.starts_with("crate::"), path.to_owned());
                if let Some(before) = &previous
                    && *before > key
                {
                    wrong.push((n + 1, format!("{path} is out of order")));
                }
                previous = Some(key);
            }
            None if line.starts_with("#[") => {}
            None => previous = None,
        }
    }
    wrong
}

#[test]
fn the_use_lines_of_the_hot_files_are_one_item_a_line_and_in_order() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/v026g/bots");
    for file in ["mod.rs", "tuning/mod.rs"] {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        let wrong = broken(&source);
        assert!(wrong.is_empty(), "{file}: {wrong:?}");
    }
}

#[test]
fn the_check_sees_a_brace_list_and_a_use_out_of_place() {
    let source = "use a::B;\nuse a::C;\n#[cfg(test)]\nuse a::D;\nuse a::{E, F};\n\nuse z::Y;\nuse crate::x::Z;\nuse b::W;\n";
    assert_eq!(
        broken(source),
        vec![
            (5, "a brace list: a::{E, F}".to_owned()),
            (8, "crate::x::Z is out of order".to_owned()),
        ]
    );
}
