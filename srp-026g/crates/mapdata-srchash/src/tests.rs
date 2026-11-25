use super::*;

fn source(files: &[(&str, &str)]) -> Source {
    let files: Vec<(String, String)> = files.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect();
    Source::parse(&files).unwrap()
}

fn digest(files: &[(&str, &str)], roots: &[&str]) -> u128 {
    let s = source(files);
    s.digest(&s.closure(roots).unwrap(), 0)
}

const GRID: &str = "
/// A grid.
pub struct WalkGrid { height: Vec<f32> }
impl WalkGrid {
    /// Builds.
    pub fn build() -> Self { let g = WalkGrid { height: vec![] }; g.erode(); crate::built::write(1); g }
    fn erode(&self) { helper(2) }
    pub fn unrelated(&self) -> u32 { 7 }
}
fn helper(x: u32) -> u32 { x + 1 }
fn other() -> u32 { 3 }
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(super::helper(1), 2); }
}
";
const BUILT: &str = "pub fn write(x: u32) -> u32 { x }";
const SIGHT: &str = "pub struct RayTree; impl RayTree { pub fn write(&self) -> u32 { crate::built::write(3) } }";

fn grid(text: &str) -> u128 {
    digest(&[("src/grid.rs", text), ("src/built.rs", BUILT), ("src/sight.rs", SIGHT)], &["WalkGrid::build"])
}

#[test]
fn comments_docs_whitespace_and_tests_change_nothing() {
    let base = grid(GRID);
    let edited = GRID
        .replace("/// Builds.", "/// Builds the grid, now documented at length.\n    // and a comment")
        .replace("{ x + 1 }", "{\n    x   +   1 // one more\n}")
        .replace("assert_eq!(super::helper(1), 2);", "assert_eq!(super::helper(5), 6);");
    assert_eq!(grid(&edited), base);
}

#[test]
fn a_token_edit_changes_only_the_roots_that_reach_it() {
    let base = grid(GRID);
    assert_ne!(grid(&GRID.replace("x + 1", "x + 2")), base, "helper is reached through erode");
    assert_eq!(grid(&GRID.replace("{ 3 }", "{ 4 }")), base, "other is not reached");
    assert_eq!(grid(&GRID.replace("{ 7 }", "{ 8 }")), base, "a method of the type that is not called");
    assert_ne!(grid(&GRID.replace("height: Vec<f32>", "height: Vec<f64>")), base, "the type a method is on");
}

#[test]
fn a_qualified_path_reaches_only_its_own_module() {
    let s = source(&[("src/grid.rs", GRID), ("src/built.rs", BUILT), ("src/sight.rs", SIGHT)]);
    let reached = s.closure(&["WalkGrid::build"]).unwrap();
    let names: BTreeSet<&str> = reached.iter().flat_map(|&u| s.units[u].names.iter().map(String::as_str)).collect();
    assert!(names.contains("built::write"), "{names:?}");
    assert!(!names.contains("RayTree::write"), "`built::write` is not RayTree's: {names:?}");
    // [R2] A method call reaches a method only once the code has the method's type: never on a type it does not name.
    let s = source(&[("src/a.rs", "fn f(t: &T) { t.write() }"), ("src/sight.rs", SIGHT), ("src/built.rs", BUILT)]);
    let reached = s.closure(&["f"]).unwrap();
    assert!(!reached.iter().any(|&u| s.units[u].names.iter().any(|n| n == "RayTree::write")));
    let s = source(&[("src/a.rs", "fn f(t: &crate::sight::RayTree) { t.write() }"), ("src/sight.rs", SIGHT), ("src/built.rs", BUILT)]);
    let reached = s.closure(&["f"]).unwrap();
    assert!(reached.iter().any(|&u| s.units[u].names.iter().any(|n| n == "RayTree::write")));
}

#[test]
fn a_trait_impl_comes_with_its_type() {
    let files = |add: &str| {
        let text = format!("pub struct V(f32); impl core::ops::Add for V {{ type Output = V; fn add(self, o: V) -> V {{ V(self.0 {add} o.0) }} }} pub fn f(a: V, b: V) -> V {{ a + b }}");
        digest(&[("src/math.rs", &text)], &["f"])
    };
    assert_ne!(files("+"), files("-"), "the operator's impl is reached by the type's name");
}

#[test]
fn self_paths_are_the_impls_type() {
    let text = "pub struct A; impl A { pub fn go() -> u32 { Self::step() } fn step() -> u32 { 1 } } pub struct B; impl B { fn step() -> u32 { 2 } }";
    let s = source(&[("src/a.rs", text)]);
    let reached = s.closure(&["A::go"]).unwrap();
    let names: BTreeSet<&str> = reached.iter().flat_map(|&u| s.units[u].names.iter().map(String::as_str)).collect();
    assert!(names.contains("A::step") && !names.contains("B::step"), "{names:?}");
}

#[test]
fn a_test_module_file_is_left_out() {
    let lib = "pub fn f() -> u32 { 1 }\n#[cfg(test)]\nmod tests;";
    let a = digest(&[("src/lib.rs", lib), ("src/tests.rs", "use super::*; #[test] fn t() { f(); }")], &["f"]);
    let b = digest(&[("src/lib.rs", lib), ("src/tests.rs", "use super::*; #[test] fn t() { f(); f(); }")], &["f"]);
    assert_eq!(a, b);
    let s = source(&[("src/lib.rs", lib), ("src/tests.rs", "pub fn g() {}")]);
    assert!(s.closure(&["g"]).is_err(), "the test file's items are not in the source");
}

#[test]
fn thread_locals_are_reached_by_their_names() {
    let a = "thread_local! { static SCRATCH: RefCell<Vec<u32>> = RefCell::new(Vec::new()); } pub fn f() { SCRATCH.with(|s| s.borrow_mut().push(1)); }";
    let b = a.replace("Vec::new()", "Vec::with_capacity(8)");
    assert_ne!(digest(&[("src/a.rs", a)], &["f"]), digest(&[("src/a.rs", &b)], &["f"]));
}

#[test]
fn unfollowed_inputs_are_errors() {
    let home = "pub fn f() -> &'static str { env!(\"HOME\") }";
    let unrooted = "pub fn f() -> &'static str { include_str!(\"x.txt\") }";
    let computed = "pub fn f() -> &'static str { include_str!(concat!(\"x\", \".txt\")) }";
    for text in [home, unrooted, computed] {
        let files = vec![("src/a.rs".to_owned(), text.to_owned())];
        assert!(Source::parse(&files).is_err(), "{text}");
    }
    let own = vec![("src/lib.rs".to_owned(), "pub mod source { include!(concat!(env!(\"OUT_DIR\"), \"/d.rs\")); }".to_owned())];
    assert!(Source::parse(&own).is_ok());
    let s = source(&[("src/a.rs", "pub fn f() {}")]);
    assert!(s.closure(&["nothing"]).is_err(), "a root that names no item");
}

#[test]
fn rustflags_cfgs_are_read_either_way() {
    assert_eq!(rustflags_cfgs(Some("-C\u{1f}opt-level=3\u{1f}--cfg\u{1f}b\u{1f}--cfg=a"), Some("--cfg z")), ["a", "b"]);
    assert_eq!(rustflags_cfgs(None, Some("--cfg z -C x")), ["z"]);
    assert_eq!(rustflags_cfgs(Some(""), None), Vec::<String>::new());
}

#[test]
fn the_hash_is_the_stream_s() {
    let mut a = Hash128::default();
    a.bytes(b"abcdefghij");
    let mut b = Hash128::default();
    b.bytes(b"abc");
    b.bytes(b"defghij");
    assert_eq!(a.clone().finish(), b.finish());
    let mut c = Hash128::default();
    c.bytes(b"abcdefghi");
    assert_ne!(a.finish(), c.finish());
}

#[test]
fn an_included_file_s_bytes_are_in_its_item() {
    let root = std::env::temp_dir().join(format!("srchash-include-{}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let text = "pub fn f() -> &'static str { include_str!(\"../data.txt\") } pub fn g() -> &'static str { env!(\"CARGO_PKG_NAME\") }";
    let files = vec![("src/a.rs".to_owned(), text.to_owned())];
    let digest = |data: &str| {
        std::fs::write(root.join("data.txt"), data).unwrap();
        let s = Source::parse_in(&files, Some(&root)).unwrap();
        (s.digest(&s.closure(&["f"]).unwrap(), 0), s.digest(&s.closure(&["g"]).unwrap(), 0))
    };
    let (f1, g1) = digest("one");
    let (f2, g2) = digest("two");
    assert_ne!(f1, f2, "the included file's bytes");
    assert_eq!(g1, g2, "an item that includes nothing");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn bindings_fields_and_foreign_paths_reach_nothing() {
    let other = "pub struct G; impl G { pub fn build() -> u32 { 1 } pub fn new() -> u32 { 2 } pub fn height(&self) -> u32 { 3 } }";
    let reaches = |text: &str| {
        let s = source(&[("src/a.rs", text), ("src/g.rs", other)]);
        let reached = s.closure(&["f"]).unwrap();
        reached.iter().flat_map(|&u| s.units[u].names.clone()).collect::<BTreeSet<String>>()
    };
    for text in [
        "pub fn f(build: u32) -> u32 { build }",
        "pub fn f() -> u32 { let build = 1; let mut new = 2; new += build; new }",
        "pub struct P { height: u32 } pub fn f(p: P) -> u32 { p.height }",
        "pub fn f() -> Vec<u32> { Vec::new() }",
    ] {
        let names = reaches(text);
        assert!(!names.contains("G::build") && !names.contains("G::new") && !names.contains("G::height"), "{text}: {names:?}");
    }
    // A call does reach, plain or through this source's type.
    assert!(reaches("pub fn f(g: G) -> u32 { g.height() }").contains("G::height"));
    assert!(reaches("pub fn f() -> u32 { G::new() }").contains("G::new"));
    // [R2] A generic parameter's call reaches a type's method once the code names the type (where the parameter is chosen).
    assert!(!reaches("pub fn f<T>() -> u32 { T::build() }").contains("G::build"), "no code names G");
    assert!(
        reaches("pub fn f() -> u32 { g::<crate::g::G>() } fn g<T>() -> u32 { T::build() }").contains("G::build"),
        "a generic parameter's call, its type named by the caller"
    );
}

/// The names of the items `roots` reach in `files`.
fn reached_names(files: &[(&str, &str)], roots: &[&str]) -> BTreeSet<String> {
    let s = source(files);
    s.closure(roots).unwrap().iter().flat_map(|&u| s.units[u].names.clone()).collect()
}

#[test]
fn r1_a_pattern_s_shorthand_binds_a_local_and_a_literal_s_names_an_item() {
    let a = "pub struct B { pub rest: u32 }
        pub fn f(b: B) -> u32 { let B { rest } = b; rest }
        pub fn m(b: Option<B>) -> u32 { match b { Some(B { rest }) => rest, None => 0 } }
        pub fn rest() -> u32 { 9 }
        #[allow(non_upper_case_globals)] pub const width: u32 = 4;
        pub struct C { pub width: u32 }
        pub fn g() -> C { C { width } }";
    assert!(!reached_names(&[("src/a.rs", a)], &["f"]).contains("a::rest"), "a let pattern's shorthand");
    assert!(!reached_names(&[("src/a.rs", a)], &["m"]).contains("a::rest"), "a match arm's shorthand");
    assert!(reached_names(&[("src/a.rs", a)], &["g"]).contains("a::width"), "a literal's shorthand is its expression");
}

#[test]
fn r2_a_method_waits_for_its_type_and_a_trait_impl_comes_with_it() {
    let a = "pub struct A; impl A { pub fn go(&self) -> u32 { 1 } }
        pub struct Z; impl Z { pub fn go(&self) -> u32 { 2 } }
        pub fn h(x: impl Fn() -> A) -> u32 { x().go() }
        pub struct V(u32);
        impl std::ops::Add for V { type Output = V; fn add(self, o: V) -> V { added(); o } }
        impl Drop for V { fn drop(&mut self) { dropped() } }
        fn added() {} fn dropped() {}
        pub fn sum(a: V, b: V) -> V { a + b }";
    let names = reached_names(&[("src/a.rs", a)], &["h"]);
    assert!(names.contains("A::go") && !names.contains("Z::go"), "an unknown receiver: the methods of the types named: {names:?}");
    let names = reached_names(&[("src/a.rs", a)], &["sum"]);
    assert!(names.contains("a::added") && names.contains("a::dropped"), "an operator and a drop name no method: {names:?}");
}

#[test]
fn r3_a_free_item_s_plain_name_resolves_in_its_module_s_scope() {
    let files = [
        ("src/lib.rs", "pub mod a; pub mod b; pub mod c; pub mod p; pub mod d;"),
        ("src/a.rs", "pub fn row() -> u32 { 1 }"),
        ("src/b.rs", "pub fn row() -> u32 { 2 } pub fn f() -> u32 { row() }"),
        ("src/c.rs", "use crate::a::row; pub fn g() -> u32 { row() }"),
        ("src/p.rs", "pub mod q; pub fn col() -> u32 { 3 }"),
        ("src/p/q.rs", "use super::*; pub fn k() -> u32 { col() }"),
        ("src/d.rs", "pub fn n() -> u32 { let row = || 4; row() } pub fn m() -> u32 { col() }"),
    ];
    let units = |root: &str| {
        let s = source(&files);
        s.closure(&[root]).unwrap().iter().map(|&u| s.units[u].file.clone()).collect::<BTreeSet<String>>()
    };
    assert!(units("f").contains("src/b.rs") && !units("f").contains("src/a.rs"), "its own module's");
    assert!(units("g").contains("src/a.rs") && !units("g").contains("src/b.rs"), "through its use");
    assert!(units("k").contains("src/p.rs"), "through a glob of its parent");
    assert!(!units("n").contains("src/a.rs") && !units("n").contains("src/b.rs"), "a local closure of the name");
    assert!(units("m").contains("src/p.rs"), "no item in scope: every item of the name (as before)");
    let s = source(&files);
    let _ = s.closure(&["m"]).unwrap();
    assert!(s.unplaced.borrow().iter().any(|n| n == "col in d"), "and the report names it: {:?}", s.unplaced.borrow());
}

#[test]
fn r4_a_method_call_on_a_known_receiver_reaches_that_type_s_methods() {
    let a = "pub struct W; impl W { pub fn hit(&self) -> u32 { 1 } }
        pub struct X; impl X { pub fn hit(&self) -> u32 { 2 } }
        pub fn make() -> X { X }
        pub struct G { w: W, b: Box<W> }
        pub trait T { fn hit2(&self) -> u32 { by_default() } }
        fn by_default() -> u32 { 5 }
        impl T for W {}
        impl G {
            pub fn on_field(&self) -> u32 { let _x: X = make(); self.w.hit() }
            pub fn on_box(&self) -> u32 { let _x: X = make(); self.b.hit() }
            pub fn on_default(&self) -> u32 { self.w.hit2() }
        }
        pub fn typed(w: &W) -> u32 { let _x: X = make(); w.hit() }
        pub fn rebound(w: &W) -> u32 { let w = make(); w.hit() }";
    let names = |root: &str| reached_names(&[("src/a.rs", a)], &[root]);
    assert!(names("G::on_field").contains("W::hit") && !names("G::on_field").contains("X::hit"), "self.field: its type's");
    assert!(names("typed").contains("W::hit") && !names("typed").contains("X::hit"), "a typed parameter: its type's");
    assert!(names("G::on_box").contains("W::hit") && names("G::on_box").contains("X::hit"), "a field's type not known: as R2");
    assert!(names("rebound").contains("X::hit"), "a name bound twice has no one type");
    assert!(names("G::on_default").contains("a::by_default"), "a trait's default method");
}

#[test]
fn inert_attributes_move_no_key_and_track_caller_does() {
    let base = grid(GRID);
    assert_eq!(grid(&GRID.replace("fn helper(", "#[inline]\n#[allow(dead_code)]\nfn helper(")), base);
    assert_eq!(grid(&GRID.replace("fn helper(", "#[must_use]\n#[cold]\nfn helper(")), base);
    assert_ne!(grid(&GRID.replace("fn helper(", "#[track_caller]\nfn helper(")), base);
}

#[test]
fn why_agrees_with_the_closure() {
    let files = [("src/grid.rs", GRID), ("src/built.rs", BUILT), ("src/sight.rs", SIGHT)];
    let s = source(&files);
    let reached = s.closure(&["WalkGrid::build"]).unwrap();
    for (at, unit) in s.units.iter().enumerate() {
        let Some(name) = unit.names.iter().find(|n| s.by_name.get(*n).is_some_and(|v| v.len() == 1)) else { continue };
        let why = s.why(&["WalkGrid::build"], name).unwrap();
        assert_eq!(why.is_some(), reached.contains(&at), "{name}");
    }
}

#[test]
fn the_legacy_resolver_reaches_every_item_of_a_name() {
    let a = "pub struct A; impl A { pub fn go(&self) -> u32 { 1 } }
        pub struct Z; impl Z { pub fn go(&self) -> u32 { 2 } }
        pub fn f(a: &A) -> u32 { a.go() }";
    let files: Vec<(String, String)> = vec![("src/a.rs".to_owned(), a.to_owned())];
    let legacy = Source::parse_with(&files, None, Resolution::Legacy).unwrap();
    let names: BTreeSet<String> = legacy.closure(&["f"]).unwrap().iter().flat_map(|&u| legacy.units[u].names.clone()).collect();
    assert!(names.contains("Z::go"), "{names:?}");
}

/// The real tree's `mapdata` files (this crate's workspace).
fn real_files() -> (std::path::PathBuf, Vec<(String, String)>) {
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = crate_files(&ws, "crates/mapdata").unwrap();
    (ws, files)
}

/// Every digest of `files` (grid, sight, each memo site's), with no outside part.
fn real_digests(ws: &Path, files: &[(String, String)]) -> BTreeMap<&'static str, u128> {
    let s = Source::parse_in(files, Some(ws)).unwrap();
    digest_roots().into_iter().map(|(what, roots)| (what, s.digest(&s.closure(roots).unwrap(), 0))).collect()
}

#[test]
fn a_builder_change_no_site_runs_moves_no_site_s_key() {
    let (ws, files) = real_files();
    let base = real_digests(&ws, &files);
    let controlled = "crates/mapdata/src/grid/controlled.rs";
    let text = &files.iter().find(|(p, _)| p == controlled).unwrap().1;
    // A new function of the builder no site calls, and a test of its own: as the bridge pass's `walked_bridge` was.
    let mut changed = files.clone();
    changed.iter_mut().find(|(p, _)| p == controlled).unwrap().1 = format!(
        "{text}\npub(super) fn walked_over(x: u32) -> u32 {{ x + 1 }}\n#[cfg(test)]\nmod more_tests {{ #[test] fn t() {{ assert_eq!(super::walked_over(1), 2); }} }}\n"
    );
    let after = real_digests(&ws, &changed);
    for (what, d) in &base {
        assert_eq!(after[what], *d, "{what}");
    }
}

#[test]
fn an_item_s_edit_moves_exactly_the_digests_whose_closure_holds_it() {
    let (ws, files) = real_files();
    let s = Source::parse_in(&files, Some(&ws)).unwrap();
    let base = real_digests(&ws, &files);
    // (file, the item's name): each edited by one more statement at the start of its body (a token change).
    let edits = [
        ("crates/mapdata/src/grid/rim.rs", "rim_aimed_job"),
        ("crates/mapdata/src/controller.rs", "Collision::build_exe"),
        ("crates/mapdata/src/grid/controlled.rs", "WalkGrid::controlled_links"),
    ];
    for (file, name) in edits {
        let unit = s.by_name[name].iter().copied().find(|&u| s.units[u].file == file).unwrap_or_else(|| panic!("{name}"));
        let text = &files.iter().find(|(p, _)| p == file).unwrap().1;
        let short = name.rsplit("::").next().unwrap();
        let at = [format!("fn {short}("), format!("fn {short}<")].iter().find_map(|sig| text.find(sig.as_str())).unwrap_or_else(|| panic!("{name}"));
        let open = at + text[at..].find('{').unwrap();
        let mut changed_text = text.clone();
        changed_text.insert_str(open + 1, " let _ = 0u8; ");
        let changed: Vec<(String, String)> =
            files.iter().map(|(p, t)| (p.clone(), if p == file { changed_text.clone() } else { t.clone() })).collect();
        let after = real_digests(&ws, &changed);
        for (what, roots) in digest_roots() {
            let holds = s.closure(roots).unwrap().contains(&unit);
            assert_eq!(after[what] != base[what], holds, "{name} in {what}'s closure: {holds}");
        }
    }
}

#[test]
fn a_use_list_is_no_hub() {
    let s = source(&[
        ("src/lib.rs", "pub use b::{One, Two};\npub fn f() -> One { One }"),
        ("src/b.rs", "pub struct One; pub struct Two;"),
    ]);
    let reached = s.closure(&["f"]).unwrap();
    let names: BTreeSet<&str> = reached.iter().flat_map(|&u| s.units[u].names.iter().map(String::as_str)).collect();
    assert!(names.contains("b::One") && !names.contains("b::Two"), "{names:?}");
}

#[test]
fn a_test_module_s_folder_is_left_out() {
    let lib = "pub fn f() -> u32 { 1 }\n#[cfg(test)]\nmod tests;";
    let s = source(&[("src/lib.rs", lib), ("src/tests/mod.rs", "mod more;"), ("src/tests/more.rs", "pub fn g() {}")]);
    assert!(s.closure(&["g"]).is_err());
}

#[test]
fn attributes_and_module_heads_reach_nothing() {
    let other = "pub fn derive() -> u32 { 1 } pub fn fmt() -> u32 { 2 }";
    let text = "#[derive(Clone)]\npub struct S; impl S { pub fn f(&self) -> fmt::Result { Ok(()) } }";
    let s = source(&[("src/a.rs", text), ("src/b.rs", other)]);
    let reached = s.closure(&["S::f"]).unwrap();
    let names: BTreeSet<&str> = reached.iter().flat_map(|&u| s.units[u].names.iter().map(String::as_str)).collect();
    assert!(!names.contains("derive") && !names.contains("fmt"), "{names:?}");
}

#[test]
fn another_crate_s_methods_wait_for_their_type() {
    let b = "pub struct Data; impl Data { pub fn len(&self) -> u32 { 1 } } impl Clone for Data { fn clone(&self) -> Self { Data } }";
    let a = "use b::Data;\npub fn f(v: Vec<u32>) -> usize { v.len() }\npub fn g(d: &Data) -> u32 { d.len() }";
    let s = source(&[("crates/a/src/lib.rs", a), ("crates/b/src/lib.rs", b)]);
    let names = |root: &str| -> BTreeSet<String> { s.closure(&[root]).unwrap().iter().flat_map(|&u| s.units[u].names.clone()).collect() };
    assert!(!names("f").contains("Data::len"), "a Vec's len is not Data's: {:?}", names("f"));
    assert!(names("g").contains("Data::len"), "{:?}", names("g"));
    // A trait impl for a type the source does not define is not reached by that type's name.
    let c = "pub trait T { fn t(&self); } impl T for Vec<u32> { fn t(&self) {} } pub fn h(v: Vec<u32>) -> usize { v.len() }";
    let s = source(&[("crates/c/src/lib.rs", c)]);
    let reached: BTreeSet<String> = s.closure(&["h"]).unwrap().iter().flat_map(|&u| s.units[u].names.clone()).collect();
    assert!(!reached.contains("Vec::t"), "{reached:?}");
}
