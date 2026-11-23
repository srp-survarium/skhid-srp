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
    // A plain call reaches every item of its name.
    let s = source(&[("src/a.rs", "fn f(t: &T) { t.write() }"), ("src/sight.rs", SIGHT), ("src/built.rs", BUILT)]);
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
    assert!(reaches("pub fn f<T>() -> u32 { T::build() }").contains("G::build"), "a generic parameter's call");
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
