//! [incremental grids] The digests of `mapdata`'s source that name its built grids and sight trees, taken at compile time from
//! the source's tokens (`crates/mapdata/build.rs`, baked into `mapdata::source`) and by the `mapdata-srchash` tool from a source
//! tree (the same code, so the two agree).
//!
//! A digest covers what can change a build's output, and nothing else:
//! - the tokens of the items a root reaches, with no comments, no doc comments, no inert attributes (lint levels, inlining hints,
//!   `#[cold]`, `#[must_use]`, rustfmt's: they change no value the code computes; `#[track_caller]` stays), no `#[cfg(test)]` or
//!   `#[test]` items and no `src/bin`, so a comment, a doc, a lint or a test edit changes no digest;
//! - the items reached are found by name as rustc resolves it, narrowed where the code shows which item a name is
//!   ([`Source::closure`], [`Resolution::Scoped`]), and as wide as before where it does not:
//!   - a `q::name` path reaches only the items of `q` (a type's own methods, a module's own items) where `q` has such an item
//!     ([`Source::resolve`]: none for a path through a type or module the source does not have, `Vec::new`);
//!   - [R3] a free item's plain name (a function, a type, a constant) resolves in its module's scope: the module's own items,
//!     its `use`s, the modules it glob-imports; a name that scope does not place reaches every item of the name, as before, and
//!     the report lists it; a macro's names resolve anywhere (where it is called);
//!   - [R4] a method call on a receiver of known type (`self`, a binding bound once with a written type, a field of either)
//!     reaches that type's methods (inherent and trait impls'), the traits that declare the name and the impls for types the
//!     source does not define; [R2] any other method call reaches a method only once the walk has reached its type: a method runs
//!     on a value of its type, made only by code that names the type. A trait impl comes with its type (an operator, a `Drop`, a
//!     `for` loop names no method);
//!   - [R1] a binding, a field and a struct pattern's shorthand (`name: T`, `let name`, `.name` not called, `let B { name } =`)
//!     and a declaration's own name reach nothing.
//!
//!   The coverage oracle (`coverage_oracle.py`, `mapdata/src/coverage.rs`) checks on a real build that every function a
//!   digest's part executes is in its closure. `--diff` names the items that moved a key, `--dropped` what the scoped resolution
//!   leaves out of each closure against the old one ([`Resolution::Legacy`]);
//! - `mapdata`'s own source only, as the cache's fingerprint before: the workspace crates it uses (`survarium`, `resources-db`,
//!   `binary-config-parser`) read the map file, whose output the grid's and the tree's keys hold. Followed by name they reach
//!   every grid through `survarium`'s content tables (its `include_str!`s), and a content edit would rebuild every grid
//!   ([`mapdata_source_wide`] reads them too, for a report). An `include_str!`/`include_bytes!` file's bytes are in the item
//!   that includes it;
//! - the inputs outside the source ([`outside`]): the `Cargo.lock` entries of `mapdata`'s dependencies, the workspace
//!   manifest's `[workspace.package]` and `[profile*]` sections, `flake.nix` (its comment lines left out) and `flake.lock`, and
//!   the `--cfg` flags of the rustflags.
//!
//! A source the parser refuses gives the fallback ([`Digests::fallback`]): every digest is the digest of every file's bytes, as
//! the cache's names were before (never fewer rebuilds, only more), and the report says so.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use proc_macro2::{Delimiter, Group, Spacing, TokenStream, TokenTree};
use quote::ToTokens;

/// The roots of `mapdata`'s digests: a built grid file is what [`GRID_ROOTS`] compute and write, a sight tree file what
/// [`SIGHT_ROOTS`] do.
pub const GRID_ROOTS: &[&str] = &["World::map_grid_with_stats", "WalkGrid::write_body", "WalkGrid::digest"];
pub const SIGHT_ROOTS: &[&str] = &["BulletMesh::build_tree", "RayTree::write", "RayTree::digest"];
/// [incremental grids PR 5] The job memo's digests ([`Digests::sites`]): each site's job, its key and its result (with their
/// codecs), and the memo's own reading and storing, so a stored result is taken only by the code that made it; `world`: the
/// game's shapes' build, whose output the flights fly on (a memo key's global part, `World::memo_global`).
pub const MEMO_SITES: &[(&str, &[&str])] = &[
    ("world", &["Collision::build_exe"]),
    ("rim.line", &["rim_line_job", "rim_line_key", "RimLineOut", "RimLineItem", "RimLineCtx", "Windows::of_floors", "Memo::run"]),
    ("rim.aimed", &["rim_aimed_job", "rim_aimed_key", "RimAimedOut", "RimAimedItem", "RimAimedCtx", "Windows::of_floors", "Memo::run"]),
    ("gap.flown", &["gap_flown_job", "gap_flown_key", "GapFlownOut", "GapFlownItem", "GapEnd", "GapFlownCtx", "Windows::of_floors", "Memo::run"]),
    ("gap.clip", &["gap_clip_job", "gap_clip_key", "GapClipOut", "GapFlownItem", "GapEnd", "GapFlownCtx", "Windows::of_floors", "Memo::run"]),
];

/// A 128-bit digest of bytes, eight a word whatever the writes' sizes, the count last. Two lanes of the word mixer
/// `mapdata::digest::Digest` uses (xor, an odd multiply, the high bits folded down), from other starts.
#[derive(Clone)]
pub struct Hash128 {
    lanes: [u64; 2],
    word: [u8; 8],
    filled: usize,
    count: u64,
}

impl Default for Hash128 {
    fn default() -> Self {
        Self { lanes: [0xcbf2_9ce4_8422_2325, 0x6c62_272e_07bb_0142], word: [0; 8], filled: 0, count: 0 }
    }
}

impl Hash128 {
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.count += bytes.len() as u64;
        for &byte in bytes {
            self.word[self.filled] = byte;
            self.filled += 1;
            if self.filled == 8 {
                self.push(u64::from_le_bytes(self.word));
                self.filled = 0;
            }
        }
    }

    /// A string, after its length: two strings end where they did.
    pub fn str(&mut self, s: &str) {
        self.u128(s.len() as u128);
        self.bytes(s.as_bytes());
    }

    pub fn u128(&mut self, v: u128) {
        self.bytes(&v.to_le_bytes());
    }

    fn push(&mut self, word: u64) {
        for (lane, turn) in self.lanes.iter_mut().zip([0u32, 29]) {
            let mixed = (*lane ^ word.rotate_left(turn)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
            *lane = mixed ^ (mixed >> 29);
        }
    }

    pub fn finish(mut self) -> u128 {
        if self.filled > 0 {
            self.word[self.filled..].fill(0);
            self.push(u64::from_le_bytes(self.word));
        }
        let count = self.count;
        self.push(count);
        (u128::from(self.lanes[0]) << 64) | u128::from(self.lanes[1])
    }
}

/// What an item's identifier names: a plain name, or `q::name`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ref {
    Plain(String),
    Qualified(String, String),
    /// A method call on a receiver of a known type: the receiver's type (`self`'s, a typed binding's), the fields read off it in
    /// order (`self.grid.world`: `grid`, `world`), the method's name.
    Method(String, Vec<String>, String),
}

/// One item of the source (a function, a method, a type with its trait impls' units, a const, a `use`, a macro...).
#[derive(Clone, Debug)]
pub struct Unit {
    /// Its file, from the workspace (`crates/mapdata/src/grid.rs`), or the crate's name for a whole crate's unit.
    pub file: String,
    /// The names it is reached by: plain (`erode`) and qualified (`WalkGrid::erode`, `grid::erode`).
    pub names: Vec<String>,
    /// Its tokens, without doc attributes, as text.
    pub text: String,
    refs: BTreeSet<Ref>,
    /// A method's or an impl's type (an impl's unit is reached from another crate by a plain name only once this type is).
    owner: Option<String>,
    /// The type a struct, enum, union, trait or type alias defines.
    defines: Option<String>,
    /// Its module (`rim` for `grid/rim.rs`, an inline `mod`'s name, `crate` for the root): where its plain names are resolved.
    module: String,
    /// A `macro_rules!` macro: its plain names resolve where it is called, so anywhere in the crate.
    macro_rules: bool,
    /// The names it binds (parameters, `let`, pattern shorthands): a plain name with no item in scope that it binds is the local.
    locals: BTreeSet<String>,
    /// Its first and last line in its file (1-based), for the coverage oracle ([`spans_report`]).
    pub lines: Option<(usize, usize)>,
}

/// The units waiting for a type, by (crate, type): (from, unit, the name it was reached by).
type Waiting<'a> = HashMap<(&'a str, &'a str), Vec<(usize, usize, String)>>;

/// A file's crate: `crates/<crate>/..` (none for a path elsewhere, as the tests' files).
fn crate_of(file: &str) -> &str {
    file.strip_prefix("crates/").and_then(|rest| rest.split('/').next()).unwrap_or("")
}

/// How plain names resolve.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resolution {
    /// [R1-R4] A pattern's shorthand binds a local; a method waits for its type; a free item's plain name in its module's scope;
    /// a method call on a receiver of known type to that type's methods. The digests' resolver.
    #[default]
    Scoped,
    /// The resolver before R1-R4 (every plain name crate-wide): kept only to list what the scoped one leaves out
    /// ([`dropped_report`]), never for a digest.
    Legacy,
}

/// A crate's (or several crates') units, by name.
#[derive(Default)]
pub struct Source {
    /// How its plain names resolve.
    pub resolution: Resolution,
    pub units: Vec<Unit>,
    by_name: HashMap<String, Vec<usize>>,
    /// The modules the source has (files and inline `mod`s): a `module::name` path whose module has no `name` item still
    /// reaches every `name` (a glob or a re-export may bring it); a path through a module or type the source does not have
    /// (`Vec::new`, `std::mem::take`) reaches nothing of the source.
    modules: BTreeSet<String>,
    /// The workspace the files' paths start from, where an `include_str!`/`include_bytes!` file is read; none: such a macro is an
    /// error.
    root: Option<PathBuf>,
    /// (crate, crate it globs from): `use other::path::*` in a crate lets its plain names reach any item of `other`.
    globs: BTreeSet<(String, String)>,
    /// Each module's parent (`rim` -> `grid`), from the files and the inline `mod`s.
    parents: HashMap<String, String>,
    /// Each module's glob imports as written (`super`, `self`, `crate` or a module's name), resolved in [`Self::index`].
    module_globs_raw: Vec<(String, String)>,
    /// Each module's glob-imported modules, transitively.
    in_scope: HashMap<String, BTreeSet<String>>,
    /// The module of the items [`Self::items`] is pushing.
    current_module: String,
    /// The plain names the module view did not place (resolved crate-wide, as before), for the report.
    pub unplaced: std::cell::RefCell<BTreeSet<String>>,
    /// Each struct's named fields' types (the last path segment, references stripped), by the struct's name; a name two structs
    /// have is in [`Self::ambiguous`] instead.
    fields: HashMap<String, HashMap<String, String>>,
    /// Struct names defined more than once: their fields are no receiver's type.
    ambiguous: BTreeSet<String>,
}

impl Source {
    /// The units of `files` (path from the workspace, text), parsed as one crate's `src`; `#[cfg(test)] mod x;` files left out.
    pub fn parse(files: &[(String, String)]) -> Result<Self, String> {
        Self::parse_in(files, None)
    }

    /// As [`Self::parse`], an `include_str!`/`include_bytes!` file read from `root` (the workspace) and its bytes' digest put in
    /// the item that includes it.
    pub fn parse_in(files: &[(String, String)], root: Option<&Path>) -> Result<Self, String> {
        Self::parse_with(files, root, Resolution::Scoped)
    }

    /// As [`Self::parse_in`], resolving plain names by `resolution`.
    pub fn parse_with(files: &[(String, String)], root: Option<&Path>, resolution: Resolution) -> Result<Self, String> {
        let mut parsed = Vec::new();
        let mut test_files = BTreeSet::new();
        for (path, text) in files {
            let file = syn::parse_file(text).map_err(|e| format!("{path}: {e}"))?;
            let module = module_of(path);
            let children = children_dir(path);
            scan_test_mods(&file.items, &children, &mut test_files);
            parsed.push((path.clone(), module, file));
        }
        let mut source = Source { root: root.map(Path::to_path_buf), resolution, ..Source::default() };
        for (path, module, file) in &parsed {
            // A test module's file, or a file under its folder (its own modules).
            let under_test = test_files.contains(path)
                || test_files.iter().any(|t| t.strip_suffix(".rs").is_some_and(|dir| path.starts_with(&format!("{dir}/"))))
                || test_files.iter().any(|t| t.strip_suffix("/mod.rs").is_some_and(|dir| path.starts_with(&format!("{dir}/"))));
            if under_test {
                continue;
            }
            source.parents.entry(module.clone()).or_insert_with(|| parent_module(path));
            source.items(path, module, None, &file.items)?;
        }
        source.index();
        Ok(source)
    }

    fn index(&mut self) {
        self.by_name.clear();
        let mut direct: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (module, target) in &self.module_globs_raw {
            let to = match target.as_str() {
                "super" => self.parents.get(module).cloned().unwrap_or_else(|| "crate".to_owned()),
                "self" => module.clone(),
                other => other.to_owned(),
            };
            direct.entry(module.clone()).or_default().insert(to);
        }
        self.in_scope.clear();
        for module in direct.keys() {
            let mut seen = BTreeSet::new();
            let mut todo: Vec<String> = direct[module].iter().cloned().collect();
            while let Some(m) = todo.pop() {
                if seen.insert(m.clone()) {
                    todo.extend(direct.get(&m).into_iter().flatten().cloned());
                }
            }
            self.in_scope.insert(module.clone(), seen);
        }
        let defined: BTreeSet<&str> = self.units.iter().filter_map(|u| u.defines.as_deref()).collect();
        for (at, unit) in self.units.iter().enumerate() {
            for name in &unit.names {
                // A trait impl for a type the source does not define (`impl MemoCodec for Vec<T>`): every `Vec` is no use of it;
                // its methods' names reach it.
                if unit.owner.as_deref() == Some(name.as_str()) && !defined.contains(name.as_str()) {
                    continue;
                }
                self.by_name.entry(name.clone()).or_default().push(at);
            }
        }
    }

    /// The walk from `roots`: every unit reached, with the unit it was reached from (none for a root) and the name. A reference
    /// reaches another crate's unit by a plain name only where its crate globs from that crate, or the unit is a method whose type
    /// the walk has reached (a type's methods wait for it): a method name (`len`, `read`) names nothing in a crate the code does
    /// not use the type of.
    fn walk(&self, roots: &[&str]) -> Result<HashMap<usize, (usize, String)>, String> {
        let mut from: HashMap<usize, (usize, String)> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        for root in roots {
            for &at in self.by_name.get(*root).ok_or_else(|| format!("the root `{root}` names no item"))? {
                from.entry(at).or_insert((usize::MAX, (*root).to_owned()));
                queue.push_back(at);
            }
        }
        // Types by (crate, name): a method waits for its own crate's type of its name, or, where its crate defines none (an impl
        // for another crate's type), for any crate's.
        let defined: BTreeSet<(&str, &str)> =
            self.units.iter().filter_map(|u| Some((crate_of(&u.file), u.defines.as_deref()?))).collect();
        let mut types: BTreeSet<(&str, &str)> = BTreeSet::new();
        let mut waiting: Waiting<'_> = HashMap::new();
        let reach = |next: usize, by: (usize, String), queue: &mut std::collections::VecDeque<usize>, from: &mut HashMap<usize, (usize, String)>| {
            if let std::collections::hash_map::Entry::Vacant(e) = from.entry(next) {
                e.insert(by);
                queue.push_back(next);
            }
        };
        while let Some(at) = queue.pop_front() {
            let unit = &self.units[at];
            if let Some(t) = unit.defines.as_deref() {
                let here = crate_of(&unit.file);
                if types.insert((here, t)) {
                    for key in [(here, t), ("", t)] {
                        for (prev, next, by) in waiting.remove(&key).unwrap_or_default() {
                            reach(next, (prev, by), &mut queue, &mut from);
                        }
                    }
                }
            }
            let here = crate_of(&unit.file);
            for r in &unit.refs {
                let Some((found, by, exact)) = self.resolve_from(r, unit) else { continue };
                let found = &found;
                // An exact path (`Reader::str`) reaches another crate's item of that path only where the unit's own crate has
                // none.
                let own_has = found.iter().any(|&u| crate_of(&self.units[u].file) == here);
                for &next in found {
                    let there = &self.units[next];
                    let other = crate_of(&there.file);
                    let open = other == here || (exact && !own_has) || self.globs.contains(&(here.to_owned(), other.to_owned()));
                    // A method reached by a plain name (a method call, a generic `T::name`) runs only on a value of its type, and
                    // every value of a type of the source is made by code naming it (its constructor, a signature, a field):
                    // it waits for its own type, in its own crate too. Its type's own name (a trait impl's unit by its type) and an
                    // exact path reach it at once.
                    let by_type_name = there.owner.as_deref() == Some(by.as_str());
                    let gated = self.resolution == Resolution::Scoped
                        && !exact
                        && !by_type_name
                        && there.owner.as_deref().is_some_and(|t| defined.contains(&(other, t)));
                    match (open && !gated, there.owner.as_deref()) {
                        (true, _) => reach(next, (at, by.clone()), &mut queue, &mut from),
                        (false, Some(t)) => {
                            // Its own crate's type of that name, else (an impl for another crate's type) any crate's.
                            let key = if defined.contains(&(other, t)) { (other, t) } else { ("", t) };
                            let ready = match key {
                                ("", t) => types.iter().any(|&(_, n)| n == t),
                                key => types.contains(&key),
                            };
                            if ready {
                                reach(next, (at, by.clone()), &mut queue, &mut from);
                            } else {
                                waiting.entry(key).or_default().push((at, next, by.clone()));
                            }
                        }
                        (false, None) => {}
                    }
                }
            }
        }
        Ok(from)
    }

    fn items(&mut self, file: &str, module: &str, in_type: Option<&str>, items: &[syn::Item]) -> Result<(), String> {
        self.modules.insert(module.to_owned());
        let outer = std::mem::replace(&mut self.current_module, module.to_owned());
        let result = self.items_in(file, module, in_type, items);
        self.current_module = outer;
        result
    }

    fn items_in(&mut self, file: &str, module: &str, in_type: Option<&str>, items: &[syn::Item]) -> Result<(), String> {
        for item in items {
            if test_only(attrs_of(item)) {
                continue;
            }
            let root_of = (module == "crate").then(|| crate_of(file).replace('-', "_")).filter(|c| !c.is_empty());
            let named = |name: &syn::Ident| {
                let mut names = vec![name.to_string(), format!("{module}::{name}")];
                // A crate root's items by the crate's name too (`survarium::Thing` from another crate).
                names.extend(root_of.iter().map(|c| format!("{c}::{name}")));
                names
            };
            match item {
                syn::Item::Fn(f) => self.push(file, named(&f.sig.ident), item, in_type)?,
                syn::Item::Const(c) => self.push(file, named(&c.ident), item, in_type)?,
                syn::Item::Static(s) => self.push(file, named(&s.ident), item, in_type)?,
                syn::Item::Struct(s) => {
                    let name = s.ident.to_string();
                    if self.fields.contains_key(&name) {
                        self.ambiguous.insert(name.clone());
                    }
                    let fields = s
                        .fields
                        .iter()
                        .filter_map(|f| Some((f.ident.as_ref()?.to_string(), type_name(&f.ty)?)))
                        .collect();
                    self.fields.insert(name, fields);
                    self.push_type(file, named(&s.ident), item, &s.ident)?;
                }
                syn::Item::Enum(e) => self.push_type(file, named(&e.ident), item, &e.ident)?,
                syn::Item::Union(u) => self.push_type(file, named(&u.ident), item, &u.ident)?,
                syn::Item::Type(t) => self.push_type(file, named(&t.ident), item, &t.ident)?,
                syn::Item::TraitAlias(t) => self.push_type(file, named(&t.ident), item, &t.ident)?,
                syn::Item::Trait(t) => {
                    let mut names = named(&t.ident);
                    for inner in &t.items {
                        if let Some(name) = trait_item_name(inner) {
                            names.push(format!("{}::{name}", t.ident));
                            names.push(name);
                        }
                    }
                    self.push_type(file, names, item, &t.ident)?;
                }
                syn::Item::Macro(m) => {
                    let names = match &m.ident {
                        Some(name) => named(name),
                        // An item macro (`thread_local!`): the names it declares.
                        None => declared_names(&m.mac.tokens),
                    };
                    self.push(file, names, item, in_type)?;
                    if m.mac.path.is_ident("macro_rules") {
                        self.units.last_mut().expect("just pushed").macro_rules = true;
                    }
                }
                syn::Item::Use(u) => {
                    // One unit a name it brings, reaching what its path names: a long `pub use` list is no hub that reaches
                    // every name in it from any one.
                    let mut leaves = Vec::new();
                    use_leaves(&u.tree, &mut Vec::new(), &mut leaves);
                    if let Some(first) = glob_root(&u.tree) {
                        self.globs.insert((crate_of(file).to_owned(), first.replace('_', "-")));
                    }
                    for target in glob_targets(&u.tree) {
                        self.module_globs_raw.push((module.to_owned(), target));
                    }
                    for (name, path) in leaves {
                        let refs: BTreeSet<Ref> = match path.as_slice() {
                            [.., q, last] => [Ref::Qualified(q.clone(), last.clone())].into(),
                            [last] => [Ref::Plain(last.clone())].into(),
                            [] => BTreeSet::new(),
                        };
                        self.units.push(Unit {
                            file: file.to_owned(),
                            names: [name.clone(), format!("{module}::{name}")]
                                .into_iter()
                                .chain(root_of.iter().map(|c| format!("{c}::{name}")))
                                .collect(),
                            text: format!("use {} as {name}", path.join("::")),
                            refs,
                            owner: None,
                            defines: None,
                            module: module.to_owned(),
                            macro_rules: false,
                            locals: BTreeSet::new(),
                            lines: token_lines(&item.to_token_stream()),
                        });
                    }
                }
                syn::Item::Mod(m) => {
                    if attrs_of(item).iter().any(|a| a.path().is_ident("path")) {
                        return Err(format!("{file}: `#[path]` on mod {}: not followed", m.ident));
                    }
                    if let Some((_, inner)) = &m.content {
                        self.parents.insert(m.ident.to_string(), module.to_owned());
                        self.items(file, &m.ident.to_string(), in_type, inner)?;
                    }
                }
                syn::Item::Impl(imp) => self.impl_items(file, imp)?,
                syn::Item::ExternCrate(e) => self.push(file, named(&e.ident), item, in_type)?,
                // Foreign blocks, verbatim tokens: reached by the names they declare.
                _ => {
                    let tokens = item.to_token_stream();
                    let names = declared_names(&tokens);
                    self.push(file, names, item, in_type)?;
                }
            }
        }
        Ok(())
    }

    fn impl_items(&mut self, file: &str, imp: &syn::ItemImpl) -> Result<(), String> {
        let ty = type_name(&imp.self_ty).unwrap_or_default();
        // The impl's head (generics, the type): each of its units carries it, so a method reaches its type.
        let mut head = TokenStream::new();
        imp.generics.to_tokens(&mut head);
        imp.self_ty.to_tokens(&mut head);
        if imp.trait_.is_some() {
            // A trait impl is one unit, reached by its type (an operator or a `Drop` names no method) and its methods' names.
            let mut names = if primitive(&ty) { Vec::new() } else { vec![ty.clone()] };
            for inner in &imp.items {
                if let Some(name) = impl_item_name(inner) {
                    names.push(format!("{ty}::{name}"));
                    names.push(name);
                }
            }
            let bound = bindings(|v| syn::visit::Visit::visit_item_impl(v, imp));
            let lines = token_lines(&imp.to_token_stream());
            return self.push_tokens(file, names, imp.to_token_stream(), Some(&ty), bound, lines);
        }
        for inner in &imp.items {
            if test_only(impl_item_attrs(inner)) {
                continue;
            }
            let names = match impl_item_name(inner) {
                Some(name) => vec![format!("{ty}::{name}"), name],
                None => Vec::new(),
            };
            let mut tokens = head.clone();
            inner.to_tokens(&mut tokens);
            let bound = bindings(|v| {
                syn::visit::Visit::visit_generics(v, &imp.generics);
                syn::visit::Visit::visit_impl_item(v, inner);
            });
            let lines = token_lines(&inner.to_token_stream());
            self.push_tokens(file, names, tokens, Some(&ty), bound, lines)?;
        }
        Ok(())
    }

    /// A type's definition: reached by its names, and once reached, its methods in other crates are reached by their names.
    fn push_type(&mut self, file: &str, names: Vec<String>, item: &syn::Item, ty: &syn::Ident) -> Result<(), String> {
        self.push(file, names, item, None)?;
        self.units.last_mut().expect("just pushed").defines = Some(ty.to_string());
        Ok(())
    }

    fn push(&mut self, file: &str, names: Vec<String>, item: &syn::Item, in_type: Option<&str>) -> Result<(), String> {
        let bound = bindings(|v| syn::visit::Visit::visit_item(v, item));
        let lines = token_lines(&item.to_token_stream());
        self.push_tokens(file, names, item.to_token_stream(), in_type, bound, lines)
    }

    fn push_tokens(
        &mut self,
        file: &str,
        names: Vec<String>,
        tokens: TokenStream,
        in_type: Option<&str>,
        bound: Bindings,
        lines: Option<(usize, usize)>,
    ) -> Result<(), String> {
        let tokens = clean(tokens);
        let mut refs = BTreeSet::new();
        let mut included = Vec::new();
        let typed = bound.typed_once();
        let mut locals = if self.resolution == Resolution::Scoped { bound.shorthand } else { BTreeSet::new() };
        collect_bindings(&tokens, &mut locals);
        let scope = Scope { in_type, locals: &locals, typed: &typed };
        collect_refs(&tokens, &scope, &mut refs, &mut included, file)?;
        let mut text = tokens.to_string();
        for path in included {
            let Some(root) = &self.root else {
                return Err(format!("{file}: includes `{path}`, read only with the workspace's root"));
            };
            let at = normalise(&Path::new(file).parent().unwrap_or(Path::new("")).join(&path));
            let bytes = std::fs::read(root.join(&at)).map_err(|e| format!("{file}: includes {at}: {e}"))?;
            let mut hash = Hash128::default();
            hash.bytes(&bytes);
            let _ = write!(text, " /* {at} {:032x} */", hash.finish());
        }
        self.units.push(Unit {
            file: file.to_owned(),
            names,
            text,
            refs,
            owner: in_type.map(str::to_owned),
            defines: None,
            module: self.current_module.clone(),
            macro_rules: false,
            locals,
            lines,
        });
        Ok(())
    }

    /// The units `roots` reach (each root a unit's name; one that names none is an error).
    pub fn closure(&self, roots: &[&str]) -> Result<BTreeSet<usize>, String> {
        Ok(self.walk(roots)?.into_keys().collect())
    }

    /// The units a reference reaches, the name it reached them by, and whether by an exact path: `q::name` where the source has
    /// it; else, when `q` is this source's (a module, a type, `crate`/`super`/`self`/`Self`, or a generic parameter: one or two
    /// letters, the first a capital), every `name`; else (`Vec::new`, `std::mem::take`) none.
    /// [`Self::resolve`] from `unit`: a plain name of a free item in the unit's module scope (the module's items, its uses,
    /// its glob imports'); a method (reached by its type) or a macro anywhere; a unit's own local where no item is in scope;
    /// else crate-wide, as before (counted in [`Self::unplaced`]).
    fn resolve_from(&self, r: &Ref, unit: &Unit) -> Option<(Vec<usize>, String, bool)> {
        if self.resolution == Resolution::Legacy {
            return self.resolve(r).map(|(found, by, exact)| (found.clone(), by, exact));
        }
        if let Ref::Method(ty, fields, name) = r {
            if let Some(found) = self.method_of(ty, fields, name) {
                return Some((found, format!("{ty}::{name}"), true));
            }
            return self.resolve_from(&Ref::Plain(name.clone()), unit);
        }
        let (found, by, exact) = self.resolve(r)?;
        if exact || !matches!(r, Ref::Plain(_)) || unit.macro_rules {
            return Some((found.clone(), by, exact));
        }
        let here = crate_of(&unit.file);
        let scope = self.in_scope.get(&unit.module);
        let placed: Vec<usize> = found
            .iter()
            .copied()
            .filter(|&c| {
                let u = &self.units[c];
                crate_of(&u.file) != here
                    || u.owner.is_some()
                    || u.macro_rules
                    || u.module == unit.module
                    || scope.is_some_and(|s| s.contains(&u.module))
            })
            .collect();
        if !placed.is_empty() {
            return Some((placed, by, exact));
        }
        if unit.locals.contains(&by) {
            return None;
        }
        self.unplaced.borrow_mut().insert(format!("{} in {}", by, unit.module));
        Some((found.clone(), by, exact))
    }

    /// [R4] The units a method call on a receiver of type `ty` (then `fields` read off it) may run: the receiver type's units
    /// named `T::name` (its inherent method, its trait impls'), the traits that declare `name` (a default method) and the impls
    /// for types the source does not define (blanket, foreign). None when a field is not found, a type is not one struct of the
    /// source, or the type has no `T::name` unit (a method of its `Deref` target, say): the plain resolution then.
    fn method_of(&self, ty: &str, fields: &[String], name: &str) -> Option<Vec<usize>> {
        let mut t = ty.to_owned();
        for f in fields {
            if self.ambiguous.contains(&t) {
                return None;
            }
            t = self.fields.get(&t)?.get(f)?.clone();
        }
        if self.ambiguous.contains(&t) {
            return None;
        }
        let own = self.by_name.get(&format!("{t}::{name}"))?;
        let defined: BTreeSet<&str> = self.units.iter().filter_map(|u| u.defines.as_deref()).collect();
        let mut found = own.clone();
        for &u in self.by_name.get(name).into_iter().flatten() {
            let unit = &self.units[u];
            let trait_declaring = unit.owner.is_none() && unit.defines.is_some();
            let foreign_impl = unit.owner.as_deref().is_some_and(|o| !defined.contains(o));
            if (trait_declaring || foreign_impl) && !found.contains(&u) {
                found.push(u);
            }
        }
        Some(found)
    }

    fn resolve(&self, r: &Ref) -> Option<(&Vec<usize>, String, bool)> {
        match r {
            Ref::Plain(name) | Ref::Method(_, _, name) => self.by_name.get(name).map(|u| (u, name.clone(), false)),
            Ref::Qualified(q, name) => {
                let key = format!("{q}::{name}");
                if let Some(units) = self.by_name.get(&key) {
                    return Some((units, key, true));
                }
                let generic = q.len() <= 2 && q.starts_with(|c: char| c.is_ascii_uppercase());
                let ours = matches!(q.as_str(), "crate" | "super" | "self" | "Self")
                    || self.by_name.contains_key(q)
                    || self.modules.contains(q)
                    || generic;
                ours.then(|| self.by_name.get(name).map(|u| (u, name.clone(), false))).flatten()
            }
        }
    }

    /// How `roots` reach the unit named `target`: the chain of units from a root, each with the name its predecessor used
    /// (a closure's explanation, for `mapdata-srchash --why`). None: not reached.
    pub fn why(&self, roots: &[&str], target: &str) -> Result<Option<Vec<String>>, String> {
        let from = self.walk(roots)?;
        let Some(&goal) = self.by_name.get(target).into_iter().flatten().filter(|u| from.contains_key(u)).min_by_key(|u| {
            let mut depth = 0;
            let mut here = **u;
            while here != usize::MAX {
                depth += 1;
                here = from[&here].0;
            }
            depth
        }) else {
            return Ok(None);
        };
        let mut chain = Vec::new();
        let mut here = goal;
        while here != usize::MAX {
            let (prev, by) = &from[&here];
            chain.push(format!("{by} -> {} ({})", self.units[here].names.first().map_or("?", String::as_str), self.units[here].file));
            here = *prev;
        }
        chain.reverse();
        Ok(Some(chain))
    }

    /// The names whose plain uses reach the most units (a closure's widest doors), with how many units each names and how
    /// many units use it, for the report.
    pub fn widest_names(&self, reached: &BTreeSet<usize>, top: usize) -> Vec<(String, usize, usize)> {
        let mut uses: HashMap<&str, usize> = HashMap::new();
        for &at in reached {
            for r in &self.units[at].refs {
                if let Ref::Plain(name) = r {
                    *uses.entry(name.as_str()).or_default() += 1;
                }
            }
        }
        let mut wide: Vec<(String, usize, usize)> = uses
            .into_iter()
            .filter_map(|(name, n)| Some((name.to_owned(), self.by_name.get(name)?.len(), n)))
            .filter(|(_, units, _)| *units > 1)
            .collect();
        wide.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        wide.truncate(top);
        wide
    }

    /// The digest of `units` (in their order in the source) and `outside`.
    pub fn digest(&self, units: &BTreeSet<usize>, outside: u128) -> u128 {
        let mut hash = Hash128::default();
        hash.u128(outside);
        for &at in units {
            hash.str(&self.units[at].file);
            hash.str(&self.units[at].text);
        }
        hash.finish()
    }

    /// Every unit.
    pub fn everything(&self) -> BTreeSet<usize> {
        (0..self.units.len()).collect()
    }
}

/// The module name a file's top-level items are qualified by: its stem (`grid/rim.rs`: `rim`), its folder's for `mod.rs`,
/// `crate` for `lib.rs` and `main.rs`.
fn module_of(path: &str) -> String {
    let p = Path::new(path);
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    match stem {
        "mod" => p.parent().and_then(|d| d.file_name()).and_then(|s| s.to_str()).unwrap_or_default().to_owned(),
        "lib" | "main" => "crate".to_owned(),
        _ => stem.to_owned(),
    }
}

/// The folder a file's `mod x;` files are in.
fn children_dir(path: &str) -> PathBuf {
    let p = Path::new(path);
    let dir = p.parent().map(Path::to_path_buf).unwrap_or_default();
    match p.file_stem().and_then(|s| s.to_str()) {
        Some("mod" | "lib" | "main") => dir,
        Some(stem) => dir.join(stem),
        None => dir,
    }
}

/// The files of the `#[cfg(test)] mod x;` declarations in `items` (in `dir`), nested inline modules too.
fn scan_test_mods(items: &[syn::Item], dir: &Path, out: &mut BTreeSet<String>) {
    for item in items {
        if let syn::Item::Mod(m) = item {
            let name = m.ident.to_string();
            match &m.content {
                None if test_only(&m.attrs) => {
                    for file in [dir.join(format!("{name}.rs")), dir.join(&name).join("mod.rs")] {
                        out.insert(file.to_string_lossy().replace('\\', "/"));
                    }
                }
                Some((_, inner)) => scan_test_mods(inner, &dir.join(&name), out),
                None => {}
            }
        }
    }
}

fn attrs_of(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// `#[test]`, or `#[cfg(test)]` exactly (a `cfg(all(test, ..))` is kept: never fewer items, only more).
fn test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || (a.path().is_ident("cfg") && matches!(&a.meta, syn::Meta::List(list) if list.tokens.to_string() == "test"))
    })
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Fn(f) => &f.attrs,
        syn::ImplItem::Const(c) => &c.attrs,
        syn::ImplItem::Type(t) => &t.attrs,
        syn::ImplItem::Macro(m) => &m.attrs,
        _ => &[],
    }
}

fn trait_item_name(item: &syn::TraitItem) -> Option<String> {
    match item {
        syn::TraitItem::Fn(f) => Some(f.sig.ident.to_string()),
        syn::TraitItem::Const(c) => Some(c.ident.to_string()),
        syn::TraitItem::Type(t) => Some(t.ident.to_string()),
        _ => None,
    }
}

fn impl_item_name(item: &syn::ImplItem) -> Option<String> {
    match item {
        syn::ImplItem::Fn(f) => Some(f.sig.ident.to_string()),
        syn::ImplItem::Const(c) => Some(c.ident.to_string()),
        syn::ImplItem::Type(t) => Some(t.ident.to_string()),
        _ => None,
    }
}

/// The last segment of a path type (`WalkGrid`, `Vec<T>`: `Vec`), through references.
fn type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        syn::Type::Reference(r) => type_name(&r.elem),
        syn::Type::Paren(p) => type_name(&p.elem),
        syn::Type::Group(g) => type_name(&g.elem),
        _ => None,
    }
}

/// Each name a `use` brings and the path it names (`use a::b::{self, C as D}`: `b` for `a::b`, `D` for `a::b::C`). A glob
/// brings no unit: the names it brings are used plain, and reach their items by name.
fn use_leaves(tree: &syn::UseTree, path: &mut Vec<String>, out: &mut Vec<(String, Vec<String>)>) {
    match tree {
        syn::UseTree::Path(p) => {
            path.push(p.ident.to_string());
            use_leaves(&p.tree, path, out);
            path.pop();
        }
        syn::UseTree::Name(n) if n.ident == "self" => {
            if let Some(last) = path.last() {
                out.push((last.clone(), path.clone()));
            }
        }
        syn::UseTree::Name(n) => {
            let mut full = path.clone();
            full.push(n.ident.to_string());
            out.push((n.ident.to_string(), full));
        }
        syn::UseTree::Rename(r) if r.rename == "_" => {}
        syn::UseTree::Rename(r) => {
            let mut full = path.clone();
            if r.ident != "self" {
                full.push(r.ident.to_string());
            }
            out.push((r.rename.to_string(), full));
        }
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| use_leaves(t, path, out)),
        syn::UseTree::Glob(_) => {}
    }
}

/// The modules a `use` glob-imports, as written: the segment before each `*` (`super`, `self`, `crate`, a module's name).
fn glob_targets(tree: &syn::UseTree) -> Vec<String> {
    fn walk(tree: &syn::UseTree, last: Option<&str>, out: &mut Vec<String>) {
        match tree {
            syn::UseTree::Path(p) => walk(&p.tree, Some(&p.ident.to_string()), out),
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| walk(t, last, out)),
            syn::UseTree::Glob(_) => out.extend(last.map(str::to_owned)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(tree, None, &mut out);
    out
}

/// A file's module's parent: the folder's module (`grid` for `src/grid/rim.rs`), `crate` for a file in `src`.
fn parent_module(path: &str) -> String {
    let p = Path::new(path);
    let dir = if p.file_stem().and_then(|s| s.to_str()) == Some("mod") { p.parent().and_then(Path::parent) } else { p.parent() };
    match dir.and_then(|d| d.file_name()).and_then(|s| s.to_str()) {
        Some("src") | None => "crate".to_owned(),
        Some(name) => name.to_owned(),
    }
}

/// The first segment of a `use` path that ends in a glob (`use survarium::v026g::*`: `survarium`).
fn glob_root(tree: &syn::UseTree) -> Option<String> {
    fn has_glob(tree: &syn::UseTree) -> bool {
        match tree {
            syn::UseTree::Glob(_) => true,
            syn::UseTree::Path(p) => has_glob(&p.tree),
            syn::UseTree::Group(g) => g.items.iter().any(has_glob),
            _ => false,
        }
    }
    match tree {
        syn::UseTree::Path(p) if has_glob(&p.tree) => Some(p.ident.to_string()),
        _ => None,
    }
}

/// A primitive type: its trait impls are reached by their methods' names, not by the type's (every `f32` is no use of them).
fn primitive(name: &str) -> bool {
    matches!(
        name,
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "f32" | "f64" | "bool"
            | "char" | "str"
    )
}

/// The names a macro's or a verbatim item's tokens declare: each identifier after `static`, `const`, `fn`, `struct`, `enum`,
/// `type`, `trait`, `mod`, `union`.
fn declared_names(tokens: &TokenStream) -> Vec<String> {
    let mut names = Vec::new();
    let mut after_keyword = false;
    for tree in tokens.clone() {
        match &tree {
            TokenTree::Ident(i) => {
                let s = i.to_string();
                if after_keyword && s != "mut" {
                    names.push(s.clone());
                    after_keyword = false;
                } else {
                    after_keyword = matches!(s.as_str(), "static" | "const" | "fn" | "struct" | "enum" | "type" | "trait" | "mod" | "union");
                }
            }
            TokenTree::Group(g) => names.extend(declared_names(&g.stream())),
            _ => after_keyword = false,
        }
    }
    names
}

/// The tokens without doc attributes (`#[doc = ..]`, `#![doc = ..]`), at every depth.
/// The first and last line `tokens` span (none for tokens with no place, as a macro's).
fn token_lines(tokens: &TokenStream) -> Option<(usize, usize)> {
    let mut span: Option<(usize, usize)> = None;
    let mut add = |s: proc_macro2::Span| {
        let (a, b) = (s.start().line, s.end().line);
        if a > 0 {
            span = Some(span.map_or((a, b), |(lo, hi)| (lo.min(a), hi.max(b))));
        }
    };
    fn walk(tokens: &TokenStream, add: &mut impl FnMut(proc_macro2::Span)) {
        for t in tokens.clone() {
            match t {
                TokenTree::Group(g) => {
                    add(g.span_open());
                    add(g.span_close());
                    walk(&g.stream(), add);
                }
                other => add(other.span()),
            }
        }
    }
    walk(tokens, &mut add);
    span
}

/// An attribute that changes no value the code computes: a doc comment, a lint level, an inlining hint, `#[cold]`, `#[must_use]`,
/// a rustfmt directive. The digest leaves it out, so adding or moving one moves no key. (`#[track_caller]` stays: code can read
/// its caller's place.)
fn inert_attribute(name: &str) -> bool {
    matches!(name, "doc" | "allow" | "expect" | "warn" | "deny" | "inline" | "cold" | "must_use" | "rustfmt")
}

fn clean(tokens: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut out = Vec::with_capacity(trees.len());
    let mut i = 0;
    while i < trees.len() {
        if let TokenTree::Punct(p) = &trees[i]
            && p.as_char() == '#'
        {
            let bang = matches!(trees.get(i + 1), Some(TokenTree::Punct(b)) if b.as_char() == '!');
            let at = i + 1 + usize::from(bang);
            if let Some(TokenTree::Group(g)) = trees.get(at)
                && g.delimiter() == Delimiter::Bracket
                && matches!(g.stream().into_iter().next(), Some(TokenTree::Ident(id)) if inert_attribute(&id.to_string()))
            {
                i = at + 1;
                continue;
            }
        }
        out.push(match &trees[i] {
            TokenTree::Group(g) => {
                let mut group = Group::new(g.delimiter(), clean(g.stream()));
                group.set_span(g.span());
                TokenTree::Group(group)
            }
            other => other.clone(),
        });
        i += 1;
    }
    out.into_iter().collect()
}

/// Every identifier of `tokens` as a [`Ref`]: `q::name` qualified (`Self` read as the impl's type), any other plain. The files
/// of `include_str!("..")` and `include_bytes!("..")` (a literal path) go to `included`; `env!` of cargo's own `CARGO_*`
/// variables and the `include!` of the build's own `OUT_DIR` file (these digests) are names; any other `include!`, `env!` or
/// `option_env!`, or an include of a path that is not a literal, is an error.
fn collect_refs(tokens: &TokenStream, scope: &Scope<'_>, out: &mut BTreeSet<Ref>, included: &mut Vec<String>, file: &str) -> Result<(), String> {
    let (in_type, locals) = (scope.in_type, scope.locals);
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (i, tree) in trees.iter().enumerate() {
        match tree {
            // An attribute's (`#[derive(Clone)]`, `#[cfg(..)]`, `#[allow(..)]`) names reach nothing: its tokens are in the text.
            TokenTree::Group(g)
                if g.delimiter() == Delimiter::Bracket
                    && (matches!(trees.get(i.wrapping_sub(1)), Some(TokenTree::Punct(p)) if p.as_char() == '#')
                        || matches!(
                            (trees.get(i.wrapping_sub(1)), trees.get(i.wrapping_sub(2))),
                            (Some(TokenTree::Punct(b)), Some(TokenTree::Punct(h))) if b.as_char() == '!' && h.as_char() == '#'
                        )) => {}
            TokenTree::Group(g) => collect_refs(&g.stream(), scope, out, included, file)?,
            TokenTree::Ident(id) => {
                let name = id.to_string();
                // A declaration's own name (`fn write`) names nothing it uses: a unit reaching every item of its own name would
                // reach every other type's method of that name.
                let declared = matches!(
                    trees.get(i.wrapping_sub(1)),
                    Some(TokenTree::Ident(k)) if matches!(
                        k.to_string().as_str(),
                        "fn" | "struct" | "enum" | "union" | "trait" | "type" | "const" | "static" | "mod"
                    )
                );
                // A binding or a field names no item: `name: T` (a parameter, a field, a struct pattern's field), `let name`,
                // `let mut name`, `.name` not called (a field read).
                let colon_after = matches!(
                    (trees.get(i + 1), trees.get(i + 2)),
                    (Some(TokenTree::Punct(p)), next) if p.as_char() == ':' && p.spacing() == Spacing::Alone
                        && !matches!(next, Some(TokenTree::Punct(n)) if n.as_char() == ':')
                );
                let after_let = match (trees.get(i.wrapping_sub(1)), trees.get(i.wrapping_sub(2))) {
                    (Some(TokenTree::Ident(k)), _) if k == "let" => true,
                    (Some(TokenTree::Ident(m)), Some(TokenTree::Ident(k))) if m == "mut" && k == "let" => true,
                    _ => false,
                };
                let dot_before = matches!(trees.get(i.wrapping_sub(1)), Some(TokenTree::Punct(p)) if p.as_char() == '.');
                let called = match trees.get(i + 1) {
                    Some(TokenTree::Group(g)) => g.delimiter() == Delimiter::Parenthesis,
                    Some(TokenTree::Punct(p)) => p.as_char() == ':',
                    _ => false,
                };
                // A name the unit binds, used and not called (`build` after `build: Body`): the binding, not an item.
                let path_after = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == ':');
                let path_before = i >= 2 && matches!(&trees[i - 1], TokenTree::Punct(p) if p.as_char() == ':');
                let local = locals.contains(&name) && !called && !path_after && !path_before;
                // A path's first segment in lower case is a module or a crate (`fmt::Result`, `std::mem`, `controlled::parallel`):
                // the path's next segment is the reference (qualified by it).
                let module_head = path_after && !path_before && name.starts_with(|c: char| c.is_ascii_lowercase());
                if declared || colon_after || after_let || (dot_before && !called) || local || module_head {
                    continue;
                }
                let bang = matches!(trees.get(i + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!');
                if bang && matches!(name.as_str(), "include" | "include_str" | "include_bytes" | "env" | "option_env") {
                    let args = trees.get(i + 2).map(|t| t.to_string()).unwrap_or_default();
                    let literal = match trees.get(i + 2) {
                        Some(TokenTree::Group(g)) => match g.stream().into_iter().collect::<Vec<_>>().as_slice() {
                            [TokenTree::Literal(l)] => {
                                let text = l.to_string();
                                text.strip_prefix('"').and_then(|t| t.strip_suffix('"')).map(str::to_owned)
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    match (name.as_str(), literal) {
                        ("include_str" | "include_bytes", Some(path)) => included.push(path),
                        ("env", Some(var)) if var.starts_with("CARGO_") => {}
                        ("include" | "env", _) if args.contains("OUT_DIR") => {}
                        _ => return Err(format!("{file}: `{name}!{args}` reads outside the tokens: not followed")),
                    }
                }
                let path_sep = i >= 3
                    && matches!(&trees[i - 1], TokenTree::Punct(p) if p.as_char() == ':')
                    && matches!(&trees[i - 2], TokenTree::Punct(p) if p.as_char() == ':' && p.spacing() == Spacing::Joint);
                let qualifier = match (path_sep, trees.get(i.wrapping_sub(3))) {
                    (true, Some(TokenTree::Ident(q))) => Some(q.to_string()),
                    _ => None,
                };
                // [R4] A method call on a receiver whose type is known: `self.m(`, `binding.m(`, `x.field.field.m(`.
                if dot_before && called && let Some((ty, fields)) = receiver(&trees, i, scope) {
                    out.insert(Ref::Method(ty, fields, name));
                    continue;
                }
                out.insert(match qualifier {
                    Some(q) if q == "Self" => match in_type {
                        Some(ty) => Ref::Qualified(ty.to_owned(), name),
                        None => Ref::Plain(name),
                    },
                    Some(q) => Ref::Qualified(q, name),
                    None => Ref::Plain(name),
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// What a unit binds, from its syntax tree.
fn bindings(visit: impl FnOnce(&mut Bindings)) -> Bindings {
    let mut v = Bindings::default();
    visit(&mut v);
    v
}

/// A unit's bindings: [R1] the shorthand fields of its struct patterns (`let Body { rest_floors, .. } = b`, `|Gap { a, .. }|`, a
/// match arm's), each binding a local of its name and naming no item (a struct literal's shorthand `Foo { x }` is the expression
/// `x`: not here); [R4] the types written for its bindings (`grid: &WalkGrid`, `let w: Windows`), how often each name is bound
/// (any pattern), and its generic parameters.
#[derive(Default)]
struct Bindings {
    shorthand: BTreeSet<String>,
    typed: HashMap<String, String>,
    bound: HashMap<String, usize>,
    generics: BTreeSet<String>,
}

impl Bindings {
    /// [R4] The bindings bound once with a written type that is not a generic parameter: their type is every use's.
    fn typed_once(&self) -> HashMap<String, String> {
        self.typed
            .iter()
            .filter(|(name, ty)| self.bound.get(*name) == Some(&1) && !self.generics.contains(*ty))
            .map(|(n, t)| (n.clone(), t.clone()))
            .collect()
    }
}

impl<'ast> syn::visit::Visit<'ast> for Bindings {
    fn visit_pat_struct(&mut self, p: &'ast syn::PatStruct) {
        for field in &p.fields {
            if field.colon_token.is_none()
                && let syn::Member::Named(name) = &field.member
            {
                self.shorthand.insert(name.to_string());
                *self.bound.entry(name.to_string()).or_default() += 1;
            }
        }
        syn::visit::visit_pat_struct(self, p);
    }
    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        *self.bound.entry(p.ident.to_string()).or_default() += 1;
        syn::visit::visit_pat_ident(self, p);
    }
    fn visit_pat_type(&mut self, p: &'ast syn::PatType) {
        if let syn::Pat::Ident(id) = &*p.pat
            && let Some(ty) = type_name(&p.ty)
        {
            self.typed.insert(id.ident.to_string(), ty);
        }
        syn::visit::visit_pat_type(self, p);
    }
    fn visit_type_param(&mut self, p: &'ast syn::TypeParam) {
        self.generics.insert(p.ident.to_string());
        syn::visit::visit_type_param(self, p);
    }
}

/// What [`collect_refs`] knows of the unit it reads: its impl's type (`Self`), its locals, its typed bindings.
struct Scope<'a> {
    in_type: Option<&'a str>,
    locals: &'a BTreeSet<String>,
    typed: &'a HashMap<String, String>,
}

/// [R4] The receiver of the method call named at `trees[at]` (after a `.`), when its type is known: `self` (the impl's type) or a
/// typed binding bound once, then the fields read off it (`self.grid.world.m(`: (`Self`'s type, [`grid`, `world`])). None for
/// any other receiver (a call's result, an index, a path, a parenthesised expression).
fn receiver(trees: &[TokenTree], at: usize, scope: &Scope<'_>) -> Option<(String, Vec<String>)> {
    let mut fields = Vec::new();
    let mut i = at.checked_sub(2)?;
    loop {
        let TokenTree::Ident(id) = &trees[i] else { return None };
        let dot_before = i >= 1 && matches!(&trees[i - 1], TokenTree::Punct(p) if p.as_char() == '.');
        if !dot_before {
            // The chain's head: not a path's segment, not a field of something else.
            if i >= 1 && matches!(&trees[i - 1], TokenTree::Punct(p) if p.as_char() == ':') {
                return None;
            }
            let name = id.to_string();
            let ty = if name == "self" { scope.in_type?.to_owned() } else { scope.typed.get(&name)?.clone() };
            fields.reverse();
            return Some((ty, fields));
        }
        fields.push(id.to_string());
        i = i.checked_sub(2)?;
    }
}

/// The names a unit binds, at every depth: `name: T` (a parameter, a struct pattern's field) and `let name`, `let mut name`.
fn collect_bindings(tokens: &TokenStream, out: &mut BTreeSet<String>) {
    let trees: Vec<TokenTree> = tokens.clone().into_iter().collect();
    for (i, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Group(g) => collect_bindings(&g.stream(), out),
            TokenTree::Ident(id) => {
                let colon_after = matches!(
                    (trees.get(i + 1), trees.get(i + 2)),
                    (Some(TokenTree::Punct(p)), next) if p.as_char() == ':' && p.spacing() == Spacing::Alone
                        && !matches!(next, Some(TokenTree::Punct(n)) if n.as_char() == ':')
                );
                let after_let = match (trees.get(i.wrapping_sub(1)), trees.get(i.wrapping_sub(2))) {
                    (Some(TokenTree::Ident(k)), _) if k == "let" => true,
                    (Some(TokenTree::Ident(m)), Some(TokenTree::Ident(k))) if m == "mut" && k == "let" => true,
                    _ => false,
                };
                // Only lower-case names: a capitalised one is a type or a constant, never a local.
                let name = id.to_string();
                if (colon_after || after_let) && name.starts_with(|c: char| c.is_ascii_lowercase() || c == '_') {
                    out.insert(name);
                }
            }
            _ => {}
        }
    }
}

/// The `.rs` files of a crate's `src`, `src/bin` left out, as (path from the workspace, text), in path order.
pub fn crate_files(workspace: &Path, crate_dir: &str) -> Result<Vec<(String, String)>, String> {
    let mut paths = Vec::new();
    let mut dirs = vec![PathBuf::from(crate_dir).join("src")];
    while let Some(dir) = dirs.pop() {
        let entries = std::fs::read_dir(workspace.join(&dir)).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = dir.join(entry.file_name());
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                if path != PathBuf::from(crate_dir).join("src").join("bin") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                paths.push(path.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|p| std::fs::read_to_string(workspace.join(&p)).map(|text| (p.clone(), text)).map_err(|e| format!("{p}: {e}")))
        .collect()
}

/// A manifest's `[dependencies]` that are workspace path crates: (identifier, folder from the workspace).
fn path_dependencies(workspace: &Path, crate_dir: &str) -> Result<Vec<(String, String)>, String> {
    let manifest = read(workspace, &format!("{crate_dir}/Cargo.toml"))?;
    let root = read(workspace, "Cargo.toml")?;
    let workspace_paths: HashMap<String, String> = section(&root, "[workspace.dependencies]")
        .filter_map(|line| {
            let (name, rest) = line.split_once('=')?;
            Some((name.trim().to_owned(), quoted_after(rest, "path")?))
        })
        .collect();
    let mut out = Vec::new();
    for line in section(&manifest, "[dependencies]") {
        let Some((name, rest)) = line.split_once('=') else { continue };
        let name = name.trim().trim_end_matches(".workspace").trim();
        let dir = if let Some(path) = quoted_after(rest, "path") {
            let joined = Path::new(crate_dir).join(path);
            Some(normalise(&joined))
        } else if rest.contains("workspace") || line.trim_start().starts_with(&format!("{name}.workspace")) {
            workspace_paths.get(name).cloned()
        } else {
            None
        };
        if let Some(dir) = dir {
            out.push((name.replace('-', "_"), dir));
        }
    }
    Ok(out)
}

/// `a/b/../c` as `a/c`.
fn normalise(path: &Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(p) => parts.push(p.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    parts.join("/")
}

fn quoted_after(text: &str, key: &str) -> Option<String> {
    let at = text.find(key)?;
    let rest = &text[at + key.len()..];
    let rest = rest.trim_start().strip_prefix('=')?.trim_start().strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_owned())
}

/// The lines of a TOML section (to the next `[`), comment and blank lines left out.
fn section<'a>(text: &'a str, header: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    text.lines()
        .skip_while(move |l| l.trim() != header)
        .skip(1)
        .take_while(|l| !l.trim_start().starts_with('['))
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
}

fn read(workspace: &Path, path: &str) -> Result<String, String> {
    std::fs::read_to_string(workspace.join(path)).map_err(|e| format!("{path}: {e}"))
}

/// The digest of the inputs outside the source (module documentation), and the files it read.
pub fn outside(workspace: &Path, package: &str, cfgs: &[String]) -> Result<(u128, Vec<String>), String> {
    let mut hash = Hash128::default();
    // The lock's entries of the package's dependencies, all depths.
    let lock = read(workspace, "Cargo.lock")?;
    let mut blocks: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
    for block in lock.split("[[package]]").skip(1) {
        let mut name = String::new();
        let mut deps = Vec::new();
        let mut in_deps = false;
        for line in block.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("name = ") {
                name = v.trim_matches('"').to_owned();
            } else if line.starts_with("dependencies = [") {
                in_deps = true;
            } else if in_deps && line.starts_with(']') {
                in_deps = false;
            } else if in_deps {
                let dep = line.trim_matches(|c| c == '"' || c == ',');
                deps.push(dep.split(' ').next().unwrap_or_default().to_owned());
            }
        }
        blocks.entry(name).or_default().push((block.trim().to_owned(), deps));
    }
    let mut todo = vec![package.to_owned()];
    let mut seen = BTreeSet::new();
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        for (_, deps) in blocks.get(&name).into_iter().flatten() {
            todo.extend(deps.iter().cloned());
        }
    }
    for name in &seen {
        for (text, _) in blocks.get(name).into_iter().flatten() {
            hash.str(text);
        }
    }
    // The workspace manifest's sections that change code: the edition, the profiles.
    let root = read(workspace, "Cargo.toml")?;
    let mut header = String::new();
    for line in root.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            header = trimmed.to_owned();
        }
        if (header == "[workspace.package]" || header.starts_with("[profile")) && !trimmed.is_empty() && !trimmed.starts_with('#') {
            hash.str(trimmed);
        }
    }
    // The toolchain.
    let flake = read(workspace, "flake.nix")?;
    for line in flake.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        hash.str(line);
    }
    hash.str(&read(workspace, "flake.lock")?);
    for cfg in cfgs {
        hash.str(cfg);
    }
    Ok((hash.finish(), ["Cargo.lock", "Cargo.toml", "flake.nix", "flake.lock"].map(str::to_owned).to_vec()))
}

/// The `--cfg` values of the rustflags cargo gives a build (`CARGO_ENCODED_RUSTFLAGS`, else `RUSTFLAGS`), sorted.
pub fn rustflags_cfgs(encoded: Option<&str>, plain: Option<&str>) -> Vec<String> {
    let flags: Vec<String> = match (encoded, plain) {
        (Some(e), _) if !e.is_empty() => e.split('\u{1f}').map(str::to_owned).collect(),
        (_, Some(p)) => p.split_whitespace().map(str::to_owned).collect(),
        _ => Vec::new(),
    };
    let mut cfgs = Vec::new();
    let mut flags = flags.into_iter();
    while let Some(flag) = flags.next() {
        if flag == "--cfg" {
            cfgs.extend(flags.next());
        } else if let Some(v) = flag.strip_prefix("--cfg=") {
            cfgs.push(v.to_owned());
        }
    }
    cfgs.sort();
    cfgs
}

/// `mapdata`'s digests.
#[derive(Clone, Debug)]
pub struct Digests {
    pub all: u128,
    pub grid: u128,
    pub sight: u128,
    /// [incremental grids PR 5] Each memo site's digest ([`MEMO_SITES`]), in its order.
    pub sites: Vec<(String, u128)>,
    /// Each root's reach and the fallback's reason, for a log.
    pub report: String,
    /// The files and folders read (a build script's rerun list).
    pub inputs: Vec<String>,
}

impl Digests {
    /// Every digest the digest of every file's bytes and the outside inputs: the names before the token digests.
    pub fn fallback(workspace: &Path, crates: &[String], why: &str) -> Result<Self, String> {
        let mut hash = Hash128::default();
        let mut inputs = Vec::new();
        for dir in crates {
            for (path, text) in crate_files(workspace, dir)? {
                hash.str(&path);
                hash.str(&text);
            }
            inputs.push(format!("{dir}/src"));
            inputs.push(format!("{dir}/Cargo.toml"));
        }
        let (outside, files) = outside(workspace, "mapdata", &[])?;
        hash.u128(outside);
        inputs.extend(files);
        let all = hash.finish();
        let sites = MEMO_SITES.iter().map(|(id, _)| ((*id).to_owned(), all)).collect();
        Ok(Digests { all, grid: all, sight: all, sites, report: format!("FALLBACK (every byte): {why}\n"), inputs })
    }
}

/// `mapdata`'s digests from the workspace at `workspace` (`srp-026g`).
pub fn mapdata_digests(workspace: &Path, cfgs: &[String]) -> Result<Digests, String> {
    let dirs = vec!["crates/mapdata".to_owned()];
    let parsed = (|| {
        let mut files = Vec::new();
        for dir in &dirs {
            files.extend(crate_files(workspace, dir)?);
        }
        Source::parse_in(&files, Some(workspace))
    })();
    let source = match parsed {
        Ok(source) => source,
        Err(why) => return Digests::fallback(workspace, &dirs, &why),
    };
    let (outside_digest, files) = outside(workspace, "mapdata", cfgs)?;
    let mut report = String::new();
    let mut digest_of = |what: &str, roots: &[&str]| -> Result<u128, String> {
        let reached = source.closure(roots)?;
        let mut by_file: BTreeMap<&str, usize> = BTreeMap::new();
        for &u in &reached {
            *by_file.entry(source.units[u].file.as_str()).or_default() += 1;
        }
        let _ = writeln!(report, "{what}: {} of {} items, roots {roots:?}", reached.len(), source.units.len());
        for (file, n) in by_file {
            let _ = writeln!(report, "  {file}: {n}");
        }
        Ok(source.digest(&reached, outside_digest))
    };
    let grid = digest_of("grid", GRID_ROOTS)?;
    let sight = digest_of("sight", SIGHT_ROOTS)?;
    let mut sites = Vec::new();
    for (id, roots) in MEMO_SITES {
        sites.push(((*id).to_owned(), digest_of(id, roots)?));
    }
    let unplaced = source.unplaced.borrow();
    let _ = writeln!(report, "unplaced plain names (resolved crate-wide): {}", unplaced.len());
    for name in unplaced.iter() {
        let _ = writeln!(report, "  unplaced {name}");
    }
    drop(unplaced);
    let all = source.digest(&source.everything(), outside_digest);
    let mut inputs: Vec<String> = dirs.iter().flat_map(|d| [format!("{d}/src"), format!("{d}/Cargo.toml")]).collect();
    inputs.extend(files);
    Ok(Digests { all, grid, sight, sites, report, inputs })
}

#[cfg(test)]
mod tests;

/// The folders of `mapdata` and the workspace crates it uses, all depths.
fn crate_dirs(workspace: &Path) -> Result<Vec<String>, String> {
    let mut dirs = vec!["crates/mapdata".to_owned()];
    let mut at = 0;
    while at < dirs.len() {
        for (_, dir) in path_dependencies(workspace, &dirs[at].clone())? {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        at += 1;
    }
    Ok(dirs)
}

/// `mapdata`'s source as the digests read it (the bin's `--why`).
/// Every digest's roots: `grid`, `sight`, then each memo site's.
pub fn digest_roots() -> Vec<(&'static str, &'static [&'static str])> {
    [("grid", GRID_ROOTS), ("sight", SIGHT_ROOTS)].into_iter().chain(MEMO_SITES.iter().copied()).collect()
}

/// A unit's name for a report: its file and its first name.
fn unit_label(u: &Unit) -> String {
    format!("{}\t{}", u.file, u.names.first().map_or("?", String::as_str))
}

/// Per digest, the items it reaches whose text differs between `a` and `b` (changed), and those only one tree's closure has:
/// why a key moved (`mapdata-srchash <tree A> --diff <tree B>`). An item is named by its file and first name.
pub fn diff_report(a: &Source, b: &Source) -> Result<String, String> {
    let mut out = String::new();
    for (what, roots) in digest_roots() {
        let texts = |s: &Source| -> Result<BTreeMap<String, Vec<String>>, String> {
            let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for u in s.closure(roots)? {
                m.entry(unit_label(&s.units[u])).or_default().push(s.units[u].text.clone());
            }
            Ok(m)
        };
        let (ta, tb) = (texts(a)?, texts(b)?);
        let mut lines = Vec::new();
        for (label, text) in &tb {
            match ta.get(label) {
                None => lines.push(format!("  added   {label}")),
                Some(old) if old != text => lines.push(format!("  changed {label}")),
                Some(_) => {}
            }
        }
        lines.extend(ta.keys().filter(|l| !tb.contains_key(*l)).map(|l| format!("  removed {l}")));
        let _ = writeln!(out, "{what}: {} of its items differ", lines.len());
        for l in lines {
            let _ = writeln!(out, "{l}");
        }
    }
    Ok(out)
}

/// Per digest, the items the legacy resolver reaches and the scoped one does not, sorted by file: what R1-R4 leave out of each
/// key, for review (`mapdata-srchash <tree> --dropped`).
pub fn dropped_report(workspace: &Path) -> Result<String, String> {
    let files = crate_files(workspace, "crates/mapdata")?;
    let scoped = Source::parse_with(&files, Some(workspace), Resolution::Scoped)?;
    let legacy = Source::parse_with(&files, Some(workspace), Resolution::Legacy)?;
    let mut out = String::new();
    for (what, roots) in digest_roots() {
        let keep: BTreeSet<String> = scoped.closure(roots)?.iter().map(|&u| unit_label(&scoped.units[u])).collect();
        let old: BTreeSet<String> = legacy.closure(roots)?.iter().map(|&u| unit_label(&legacy.units[u])).collect();
        let dropped: Vec<&String> = old.iter().filter(|l| !keep.contains(*l)).collect();
        let added: Vec<&String> = keep.iter().filter(|l| !old.contains(*l)).collect();
        let _ = writeln!(out, "{what}: legacy {} items, scoped {}, dropped {}, added {}", old.len(), keep.len(), dropped.len(), added.len());
        for l in dropped {
            let _ = writeln!(out, "  dropped {l}");
        }
        for l in added {
            let _ = writeln!(out, "  added   {l}");
        }
    }
    Ok(out)
}

/// Every unit of the tree's `mapdata`: its file, first and last line, first name, and the digests whose closure holds it
/// (comma-separated, `-` for none), tab-separated, for the coverage oracle (`mapdata-srchash <tree> --spans`).
pub fn spans_report(workspace: &Path) -> Result<String, String> {
    let s = mapdata_source(workspace)?;
    let mut holders: Vec<Vec<&str>> = vec![Vec::new(); s.units.len()];
    for (what, roots) in digest_roots() {
        for u in s.closure(roots)? {
            holders[u].push(what);
        }
    }
    let mut out = String::new();
    for (u, unit) in s.units.iter().enumerate() {
        let (a, b) = unit.lines.unwrap_or((0, 0));
        let held = if holders[u].is_empty() { "-".to_owned() } else { holders[u].join(",") };
        let _ = writeln!(out, "{}\t{a}\t{b}\t{}\t{held}", unit.file, unit.names.first().map_or("?", String::as_str));
    }
    Ok(out)
}

pub fn mapdata_source(workspace: &Path) -> Result<Source, String> {
    Source::parse_in(&crate_files(workspace, "crates/mapdata")?, Some(workspace))
}

/// `mapdata` and the workspace crates it uses, all depths, item by item: what the digests leave out, for a report.
pub fn mapdata_source_wide(workspace: &Path) -> Result<Source, String> {
    let mut files = Vec::new();
    for dir in crate_dirs(workspace)? {
        files.extend(crate_files(workspace, &dir)?);
    }
    Source::parse_in(&files, Some(workspace))
}
