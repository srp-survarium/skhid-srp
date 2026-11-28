//! The base pre-warm set: the pre-warm entries common to the served projects' lists, queried once in the base process
//! before the cut, so every zygote shares them instead of querying them again after its own level.
//!
//! * **The set** (front, [`base_set`]): an entry joins when at least two of the served projects' own pre-warm lists
//!   hold it and, with a cap on loaded maps, its share of the lists is at least 1.5 / `max_zygotes` (the set pays once
//!   about two loaded maps use it), minus the entries of the exclusion file. Computed at every base start and written
//!   to the base's own folder.
//! * **The base** ([`base_prewarm_from_args`], end of `run::step3_base`): queries the set through the zygote pre-warm's
//!   own query, and reads the exe's name cache (`registry`) before the set, after every entry that changed the cache's
//!   count, and at the end: the cache entries the set added, and which entries added each of them.
//! * **The zygote** ([`check_start`] / [`check_end`]): from the start of its level load until its own pre-warm starts,
//!   an execution breakpoint at the cache lookup's branch for a found entry (`bp::FOUND_AT`) looks every found entry up
//!   among those the set added. None found: the level load met the cache state it meets without the set, and the
//!   zygote's own pre-warm skips the set's entries (its top-up too). One found (or a check that could not run): the
//!   zygote serves no match; it adds the set entries that added that cache entry to the exclusion file and starts again
//!   on its own, without the set (exec of its own command line, its control socket kept, so a queued hand-over waits
//!   for it). The front starts a new base without those entries once the exclusion file changed.
//! * **The lists**: a match on such a zygote finds the set's entries cached, so its creation never asks for what their
//!   cooks ask for; the recorder adds those entries to the list as a cold creation would have recorded them
//!   ([`with_closure`]).
//!
//! Diagnosis: `HOST_BASE_PREWARM=0` in the world host's environment: no set (A/B runs).

use crate::hlog;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Diagnosis only: `HOST_BASE_PREWARM=0` in the front's environment (the base inherits it): no base set.
pub fn off() -> bool {
    std::env::var("HOST_BASE_PREWARM").is_ok_and(|v| v == "0")
}

/// The share rule: an entry held by `holding` of `lists` lists joins the set when at least two lists hold it and, with
/// a cap on loaded maps (`max_zygotes` > 0), `holding / lists >= 1.5 / max_zygotes`.
pub fn joins(holding: usize, lists: usize, max_zygotes: u32) -> bool {
    holding >= 2 && (max_zygotes == 0 || 2 * holding as u64 * u64::from(max_zygotes) >= 3 * lists as u64)
}

/// The fewest lists that must hold an entry for it to join (None: no entry can join).
pub fn lists_needed(lists: usize, max_zygotes: u32) -> Option<usize> {
    (2..=lists).find(|&h| joins(h, lists, max_zygotes))
}

/// The base set of `lists` ([`joins`]): each entry once, in the order it first appears (lists in the given order),
/// without the entries of `exclude`. Returns the set and how many entries the exclusion took out.
pub fn base_set(lists: &[Vec<(u32, String)>], max_zygotes: u32, exclude: &HashSet<(u32, String)>) -> (Vec<(u32, String)>, usize) {
    let mut count: HashMap<&(u32, String), usize> = HashMap::new();
    let mut order: Vec<&(u32, String)> = Vec::new();
    for l in lists {
        let mut once = HashSet::new();
        for e in l {
            if once.insert(e) {
                let c = count.entry(e).or_insert(0);
                if *c == 0 {
                    order.push(e);
                }
                *c += 1;
            }
        }
    }
    let (mut out, mut excluded) = (Vec::new(), 0);
    for e in order {
        if !joins(count[e], lists.len(), max_zygotes) {
            continue;
        }
        if exclude.contains(e) {
            excluded += 1;
        } else {
            out.push(e.clone());
        }
    }
    (out, excluded)
}

/// What the base's pre-warm left in the name cache (set before the cut; every zygote and match child inherits it).
#[derive(Debug, Default)]
pub struct BaseSet {
    /// the entries queried, in their order
    pub entries: Vec<(u64, String)>,
    /// the cache nodes the set added, ascending (the zygote's check looks found nodes up here)
    pub nodes: Vec<u64>,
    /// per node of `nodes` (same order): class, name, and the first and last entry whose queries may have added it
    pub added: Vec<(u32, String, usize, usize)>,
    /// the exclusion file a zygote grows when its check finds one of `nodes`
    pub exclude: Option<PathBuf>,
    /// why `nodes` is not known (a walk of the cache failed): every zygote of this base starts again on its own
    pub unknown: Option<String>,
}

static BASE: OnceLock<BaseSet> = OnceLock::new();

/// The base set of this process (inherited from the base by fork), if any.
pub fn get() -> Option<&'static BaseSet> {
    BASE.get()
}

/// Walks of the cache wait for its lock up to this many times (each walk waits up to 50 ms).
const WALK_TRIES: u32 = 20;

/// One walk of the exe's name cache (`registry::walk_nodes`), tried again while the lock is busy.
fn walk_retry(mut f: impl FnMut(u64, u32, &[u8])) -> Option<crate::registry::Walk> {
    for _ in 0..WALK_TRIES {
        if let Some(w) = crate::registry::walk_nodes(&mut f) {
            return Some(w);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

/// The base process, at the end of `run::step3_base` (before the cut): `--base-prewarm FILE` (the set the front wrote)
/// queried entry by entry, the cache read before, during and after it, the result kept for the zygotes
/// (`--base-exclude FILE`: where they record a hit). Nothing without `--base-prewarm` or with `HOST_BASE_PREWARM=0`.
pub fn base_prewarm_from_args() {
    let Some(path) = crate::cmdline::value("--base-prewarm") else { return };
    if off() {
        hlog!("base pre-warm: off (diagnosis HOST_BASE_PREWARM=0)");
        return;
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            hlog!("base pre-warm: {path}: {e}; no set");
            return;
        }
    };
    let entries: Vec<(u64, String)> = crate::daemon::prewarm_lines(&text).map(|(c, n)| (c, n.to_string())).collect();
    if entries.is_empty() {
        hlog!("base pre-warm: {path} has no entry; no set");
        return;
    }
    let key = crate::registry::key;
    let mut before = HashSet::new();
    let Some(w0) = walk_retry(|_, c, n| {
        before.insert(key(c, n));
    }) else {
        hlog!("base pre-warm: not done: the name cache's lock was not free before the set");
        return;
    };
    let t0 = Instant::now();
    // every key seen so far, and for each key the set added: the entries queried since the walk before it
    let mut seen = before.clone();
    let mut by: HashMap<u64, (usize, usize)> = HashMap::new();
    let (mut from, mut last, mut walks, mut busy) = (0usize, w0.count, 0u32, 0u32);
    let (mut kept, mut empty) = (0u32, 0u32);
    let mut empty_names = Vec::new();
    for (i, (c, n)) in entries.iter().enumerate() {
        if crate::run::query_kept(n, *c) {
            kept += 1;
        } else {
            empty += 1;
            if empty_names.len() < 16 {
                empty_names.push(format!("{c:#x} {n}"));
            }
        }
        if crate::registry::count() == last {
            continue;
        }
        let mut new = Vec::new();
        match crate::registry::walk_nodes(|_, c, n| {
            let k = key(c, n);
            if !seen.contains(&k) {
                new.push(k);
            }
        }) {
            Some(w) => {
                walks += 1;
                last = w.count;
                for k in new {
                    seen.insert(k);
                    by.insert(k, (from, i));
                }
                from = i + 1;
            }
            // the next walk names this entry's keys with the later ones (a wider range of entries)
            None => busy += 1,
        }
    }
    let ms = t0.elapsed().as_millis();
    let end = entries.len() - 1;
    let mut found: Vec<(u64, u32, String, usize, usize)> = Vec::new();
    let w = walk_retry(|node, c, n| {
        let k = key(c, n);
        if !before.contains(&k) {
            let (a, b) = by.get(&k).copied().unwrap_or((from.min(end), end));
            found.push((node, c, String::from_utf8_lossy(n).into_owned(), a, b));
        }
    });
    let unknown = match w {
        None => Some("the name cache's lock was not free after the set".to_string()),
        Some(w) if w.bad > 0 || w.cut => Some(format!("the walk after the set skipped {} unreadable node(s) or name(s)", w.bad)),
        Some(_) => None,
    };
    found.sort_by_key(|f| f.0);
    let nodes: Vec<u64> = found.iter().map(|f| f.0).collect();
    let added = found.into_iter().map(|(_, c, n, a, b)| (c, n, a, b)).collect();
    hlog!(
        "base pre-warm: {} entries ({kept} kept, {empty} empty) in {ms} ms from {path}; the set added {} name-cache entries to the {} \
         before it ({walks} walks{})",
        entries.len(),
        nodes.len(),
        w0.count,
        if busy > 0 { format!(", {busy} skipped: the lock was busy") } else { String::new() }
    );
    if !empty_names.is_empty() {
        hlog!("base pre-warm: empty entries (first {}): {}", empty_names.len(), empty_names.join(", "));
    }
    if let Some(u) = &unknown {
        hlog!("base pre-warm: the entries the set added are not known ({u}): every zygote of this base starts again on its own");
    }
    crate::run::mem_snapshot("base: after the base pre-warm");
    let exclude = crate::cmdline::value("--base-exclude").map(PathBuf::from);
    let _ = BASE.set(BaseSet { entries, nodes, added, exclude, unknown });
}

/// The breakpoints of a running check and when it started.
static CHECK: Mutex<Option<(Vec<i32>, usize, Instant)>> = Mutex::new(None);

/// A zygote forked from a base with a set, at the start of its level load: the check of every found cache entry
/// (`bp::arm_found_check`) until [`check_end`]. Nothing when the set added no cache entry or is not known.
pub fn check_start() {
    let Some(b) = get() else { return };
    if b.unknown.is_some() || b.nodes.is_empty() {
        return;
    }
    let (fds, threads) = crate::bp::arm_found_check(&b.nodes);
    *CHECK.lock().unwrap_or_else(|e| e.into_inner()) = Some((fds, threads, Instant::now()));
}

/// The entries behind the found nodes `hits` (indexes into `b.entries`, each once, ascending), and one line per node.
pub fn hit_entries(b: &BaseSet, hits: &[u64]) -> (BTreeSet<usize>, Vec<String>) {
    let mut roots = BTreeSet::new();
    let mut lines = Vec::new();
    let mut once = HashSet::new();
    for node in hits {
        let Ok(i) = b.nodes.binary_search(node) else { continue };
        if !once.insert(i) {
            continue;
        }
        let (c, n, a, z) = &b.added[i];
        roots.extend(*a..=*z);
        let names: Vec<String> = (*a..=*z).filter_map(|r| b.entries.get(r)).map(|(c, n)| format!("{c:#x} {n}")).collect();
        lines.push(format!("{c:#x} {n} (added by {})", names.join(", ")));
    }
    (roots, lines)
}

/// A zygote, right before its own pre-warm: the end of the check. Returns when the zygote may serve matches; when its
/// level load found an entry the base set added (or the check could not run) it never returns: the entries behind it
/// join the exclusion file and the zygote starts again on its own, without the set.
pub fn check_end() {
    let Some(b) = get() else { return };
    if let Some(u) = &b.unknown {
        restart_on_own(&format!("the base set's cache entries are not known ({u})"));
    }
    let Some((fds, threads, t0)) = CHECK.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let armed = fds.len();
    let (found, hits, n_hits) = crate::bp::end_found_check(fds);
    if armed < threads {
        restart_on_own(&format!("level-load check armed on {armed} of {threads} threads"));
    }
    if n_hits == 0 {
        hlog!(
            "zygote: level-load check: {found} lookups found a cached entry, none of the {} the base set added ({} ms, {armed} threads)",
            b.nodes.len(),
            t0.elapsed().as_millis()
        );
        return;
    }
    let (roots, lines) = hit_entries(b, &hits);
    hlog!("zygote: level-load check: {n_hits} of {found} lookups found entries the base set added");
    for l in &lines {
        hlog!("zygote: level-load check: found {l}");
    }
    let text: String = roots.iter().filter_map(|&r| b.entries.get(r)).map(|(c, n)| format!("{c:#x}\t{n}\n")).collect();
    match &b.exclude {
        Some(p) => match crate::daemon::grow_prewarm_list(p, &text) {
            Ok((n, added)) => hlog!("zygote: base set exclusion {}: {} entries, {added} new ({n} in the file)", p.display(), roots.len()),
            Err(e) => hlog!("zygote: base set exclusion {} not written: {e}", p.display()),
        },
        None => hlog!("zygote: no base set exclusion file"),
    }
    restart_on_own(&format!("its level load found {n_hits} entries the base set added"));
}

/// The zygote serves no match: it runs its own command line again as a new program (the same process, so the front's
/// pidfd and the base's wait stay right), with its output in `<out>/own` and its control socket kept open, so the
/// hand-over already queued is served by a zygote that loaded everything itself, without the base set.
fn restart_on_own(why: &str) -> ! {
    use std::os::unix::process::CommandExt;
    let mut a = crate::cmdline::args();
    let own = format!("{}/own", crate::run::out_dir());
    crate::cmdline::set_value(&mut a, "--out", &own);
    if let Some(fd) = crate::cmdline::value_in(&a, "--zygote-fd").and_then(|v| v.parse::<i32>().ok()) {
        // SAFETY: the zygote's own control socket; only its close-on-exec flag changes
        unsafe { libc::fcntl(fd, libc::F_SETFD, 0) };
    }
    hlog!("zygote: {why}: it serves no match and starts again on its own, without the base set (log {own}/host.log)");
    crate::log::flush();
    let e = match std::env::current_exe() {
        Ok(exe) => std::process::Command::new(exe).args(a.iter().skip(1)).exec(),
        Err(e) => e,
    };
    hlog!("zygote: starting again on its own failed: {e}; exiting");
    crate::log::flush();
    unsafe { libc::_exit(31) }
}

/// The set's entries a zygote skips in its own pre-warm and top-up (empty without a set).
pub fn entries() -> HashSet<(u64, String)> {
    get().map(|b| b.entries.iter().cloned().collect()).unwrap_or_default()
}

/// [`with_closure`] on a given set: the list text `list` plus, for every entry of it that is an entry of the set, the
/// cache entries the set added through it of the classes a zygote can query by name
/// ([`crate::daemon::PREWARM_CLASSES`]), as list lines after the list's own (each once, none twice).
pub fn with_closure_of(list: &str, b: &BaseSet) -> String {
    let index: HashMap<(u64, &str), usize> = b.entries.iter().enumerate().map(|(i, (c, n))| ((*c, n.as_str()), i)).collect();
    let roots: HashSet<usize> = crate::daemon::prewarm_lines(list).filter_map(|(c, n)| index.get(&(c, n)).copied()).collect();
    if roots.is_empty() {
        return list.to_string();
    }
    let mut extra = String::new();
    for (c, n, a, z) in &b.added {
        let listable = crate::daemon::PREWARM_CLASSES.contains(c) && !n.is_empty() && n.len() < 119 && !n.contains(['\t', '\r', '\n', '\0']);
        if listable && (*a..=*z).any(|r| roots.contains(&r)) {
            extra.push_str(&format!("{c:#x}\t{n}\n"));
        }
    }
    crate::daemon::prewarm_union(list, &extra).0
}

/// A match creation's recorded list (`bp::record_prewarm`) with what the base set's cooks asked for: such entries are
/// found cached, so their cooks' own requests never show in the creation; a cold creation records them. Unchanged
/// without a set.
pub fn with_closure(list: &str) -> String {
    match get() {
        Some(b) => with_closure_of(list, b),
        None => list.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(c: u32, n: &str) -> (u32, String) {
        (c, n.to_string())
    }

    #[test]
    fn the_share_rule_follows_max_zygotes() {
        // 15 lists: 2 maps -> 12 lists, 5 -> 5, 12 or more -> 2; no cap -> 2
        assert_eq!(lists_needed(15, 2), Some(12));
        assert_eq!(lists_needed(15, 5), Some(5));
        assert_eq!(lists_needed(15, 12), Some(2));
        assert_eq!(lists_needed(15, 40), Some(2));
        assert_eq!(lists_needed(15, 0), Some(2));
        assert_eq!(lists_needed(15, 1), None, "one map: nothing is shared");
        assert_eq!(lists_needed(1, 0), None, "one list: nothing is in two");
        assert!(joins(2, 2, 2) && !joins(1, 2, 2));
        assert!(joins(3, 4, 2) && !joins(2, 4, 2), "3 of 4 = 0.75 = 1.5 / 2; 2 of 4 is less");
    }

    #[test]
    fn the_set_keeps_first_order_counts_each_list_once_and_drops_exclusions() {
        let lists = vec![
            vec![e(0x1f, "a"), e(0x31, "b"), e(0x1f, "a"), e(0x3, "c")],
            vec![e(0x31, "b"), e(0x1f, "a"), e(0x2c, "d")],
            vec![e(0x2c, "d"), e(0x3, "c"), e(0x31, "b")],
        ];
        let none = HashSet::new();
        assert_eq!(base_set(&lists, 0, &none), (vec![e(0x1f, "a"), e(0x31, "b"), e(0x3, "c"), e(0x2c, "d")], 0));
        // max_zygotes 2: 1.5 / 2 of 3 lists = 2.25 -> held by all three only
        assert_eq!(base_set(&lists, 2, &none), (vec![e(0x31, "b")], 0));
        // "a" twice in the first list counts once: in 2 lists, not 3
        let x: HashSet<(u32, String)> = [e(0x31, "b")].into_iter().collect();
        assert_eq!(base_set(&lists, 2, &x), (vec![], 1));
        assert_eq!(base_set(&lists, 0, &x), (vec![e(0x1f, "a"), e(0x3, "c"), e(0x2c, "d")], 1));
        assert_eq!(base_set(&[], 0, &none), (vec![], 0));
    }

    fn set() -> BaseSet {
        BaseSet {
            entries: vec![(0x31, "anim".into()), (0x1f, "cfg".into()), (0xd4, "loc".into())],
            nodes: vec![0x100, 0x200, 0x300, 0x400],
            added: vec![
                (0x31, "anim".into(), 0, 0),
                (0x2f, "anim".into(), 0, 0),
                (0x1f, "anim.bones".into(), 0, 0),
                (0x1f, "shared".into(), 1, 2),
            ],
            exclude: None,
            unknown: None,
        }
    }

    #[test]
    fn a_found_node_names_the_entries_that_added_it() {
        let b = set();
        let (roots, lines) = hit_entries(&b, &[0x400, 0x999, 0x400, 0x200]);
        assert_eq!(roots.into_iter().collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(lines, vec!["0x1f shared (added by 0x1f cfg, 0xd4 loc)".to_string(), "0x2f anim (added by 0x31 anim)".to_string()]);
        assert!(hit_entries(&b, &[0x999]).0.is_empty(), "a node the set did not add is no hit");
    }

    #[test]
    fn a_recorded_list_gets_what_the_set_entries_it_holds_asked_for() {
        let b = set();
        // the creation asked for the animation (found cached): its own sub-entries of list classes join, 0x2f does not
        let l = with_closure_of("0x31\tanim\n0x3\tother\n", &b);
        assert_eq!(l, "0x31\tanim\n0x3\tother\n0x1f\tanim.bones\n");
        // an entry whose range holds the asked-for entry joins too; nothing twice
        let l = with_closure_of("0xd4\tloc\n0x1f\tshared\n", &b);
        assert_eq!(l, "0xd4\tloc\n0x1f\tshared\n");
        assert_eq!(with_closure_of("0x3\tother\n", &b), "0x3\tother\n", "no set entry: unchanged");
    }
}
