//! The `mod` lists of the files every feature adds a module to stay in alphabetical order, so that two PRs
//! that each add a module touch two lines apart and not the same one. `cargo fmt` would keep them
//! (rustfmt's `reorder_modules`), but the repo is not rustfmt-clean, so a fmt run over these files would itself
//! conflict with every open PR; this test says it instead, and names the line.
//!
//! A *list* is a run of `mod` lines (with their `#[cfg]`/`#[path]` attributes) with nothing else between them; a
//! blank line, a comment or another item starts a new list, as in rustfmt. Enums (`PlayKind`, `Input`) are
//! not sorted: their order is part of their meaning (`PlayKind` is a `BTreeMap` key, so its order decides
//! the order plays are visited in).

use std::path::Path;

/// The names of the `mod` lists of `source` that are not in order, each as `(first wrong name, line)`.
fn unsorted(source: &str) -> Vec<(String, usize)> {
    let mut wrong = Vec::new();
    let mut previous: Option<String> = None;
    for (n, line) in source.lines().enumerate() {
        let line = line.trim_end();
        let name = line
            .strip_prefix("pub(crate) ")
            .or_else(|| line.strip_prefix("pub(super) "))
            .or_else(|| line.strip_prefix("pub "))
            .unwrap_or(line)
            .strip_prefix("mod ")
            .and_then(|rest| rest.strip_suffix(';'))
            .map(str::to_owned);
        match name {
            Some(name) => {
                if let Some(before) = &previous
                    && *before > name
                {
                    wrong.push((name.clone(), n + 1));
                }
                previous = Some(name);
            }
            // An attribute belongs to the `mod` after it: the list goes on.
            None if line.starts_with("#[") => {}
            None => previous = None,
        }
    }
    wrong
}

#[test]
fn the_lists_of_modules_are_in_order() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/v026g/bots");
    for file in [
        "mod.rs",
        "plays/mod.rs",
        "tests/mod.rs",
        "tests/tallies/owner/mod.rs",
        "tuning/mod.rs",
    ] {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        let wrong = unsorted(&source);
        assert!(wrong.is_empty(), "{file}: `mod` out of alphabetical order at (name, line) {wrong:?}");
    }
}

#[test]
fn the_check_sees_a_module_out_of_place() {
    let source = "mod b;\n#[cfg(test)]\nmod c;\nmod a;\n\nmod z;\n// a comment starts a list\nmod y;\npub mod z;\nmod zz;\n";
    assert_eq!(unsorted(source), vec![("a".to_owned(), 4)]);
}
