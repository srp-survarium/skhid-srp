//! `srp-world-host`: the world host daemon.
//!
//! Four kinds of process, all this binary:
//! * **front** (`world-host daemon --config FILE`): listens on the unix socket, reads each connection's first frame
//!   (`Create`), and hands the connection (descriptor + the frame) to the zygote of the match's level project,
//!   loading that zygote on first use through the base process. It never loads the exe.
//! * **base** (started by the front with `--base-fd 3`, [`base_loop`]): boots the exe once and runs the
//!   level-independent start-up (steps 1-2, step 3 up to the cut: `run::step3_base`), then `fork()`s one zygote per
//!   map on the front's request ([`crate::base`]) and answers with the zygote's pid and a pidfd. Every zygote shares the
//!   base's start-up pages. One that ended is started again (at most every 5 s); running zygotes do not depend on it.
//! * **zygote** (a fork of the base, its own command line, log and output folder): loads one level project with the
//!   null render and the null cooks, queries its pre-warm list, then waits for hand-overs and `fork()`s one child per
//!   match. It never creates a match itself (one match per process: the core is the static 0x20F442760). The front
//!   watches and kills it through its pidfd; the base reaps it.
//! * **child**: takes over the exe threads (`run::fork_world`), creates the match from `Create` (client mode,
//!   [`crate::world::MatchWorld`]), answers `Ready`, then runs [`crate::session::Session`] on the connection until
//!   `Close`, a fault or EOF (EOF = the relay is gone: the child exits). One connection = one match: the child's exit
//!   is the relay's EOF.
//!
//! Limits: at most `max_matches` children per zygote, more (up to twice it) only while the machine has
//! `OVER_MATCHES_FREE_MB` available (else refused with `Fault`, `match_room`); at most `max_zygotes` zygotes
//! (0 = no cap; the base process is not a map and does not count) counting every one that still exists, also an evicted one that has
//! not exited yet (`plan`). A map never has two zygotes. A match for a map that is not loaded, with no room, evicts
//! idle zygotes (no match running, every hand-over taken: each zygote reports that on its control socket,
//! [`crate::fdpass::send_status`]), the least recently used first and `prestart` maps last, by closing their control
//! socket; it never evicts a busy one (its matches share its pages, and it would free nothing until they ended). With no
//! idle zygote to evict, while the machine has `OVER_CAP_FREE_MB` available, the map loads one over the limit (the next
//! new map evicts back down to it); else the match waits up to `ROOM_WAIT` (20 s) for room, then is refused (for a base
//! process that is not ready, or refused a fork for now, up to `BASE_START_WAIT`, 30 s). Every load,
//! eviction, wait and refusal is logged with the counts. A child's `oom_score_adj` is 500, 800 for a match started
//! over max_matches (an OOM kills a match, such an extra one first, not the zygote); a child has a wall-clock backstop (SIGALRM) and a read
//! timeout; an idle zygote exits after `idle_secs` and is started again on demand. Every fork() parks the engine's
//! worker poller in the Sleep stand-in first ([`crate::k32::fork_gate_close`]) and waits for the exe's timer-loop
//! threads to be in their idle wait (`run::fork_world`, the fork census). Idle parking between requests
//! (`k32::fork_gate_idle`) lets the base's worker poller (`:sound`) sleep 25 ms per turn; a zygote restarts only
//! `:hdd` and `:resources manager` after its fork, so it has no worker poller.
//!
//! Room status: a `ListStatus` connection gets the front's view of the limits ([`room_status`]): every loaded map with
//! its running and starting matches, whether it takes one more (`match_room`, as the zygote decides), its rank in the
//! eviction order when idle and its age, and the maps that are not loaded and can load now, each with the idle maps
//! evicted for it (`plan` for a new map, again and again; none while the base process is not ready), and while no new
//! map can load, the map the queues should drain ([`DrainTurn`]). Creates the front has read but not handed over count as starting. The lobby reads it about every 0.5 s and
//! starts a queue match on a map that fits instead of one this front would refuse.

use crate::hlog;
use crate::session::{Session, SessionConfig};
use std::collections::HashMap;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use world_host_proto::{read_frame, Frame, FromHost, Ready, ToHost, MIN_PROTO_VERSION, PROTO_VERSION};

/// One level project the daemon serves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectCfg {
    /// the options' project name (`pve_01_v1_maze2`, `level_01_day_tdm`): the key of `Create.project`
    pub name: String,
    /// the level the zygote loads (`pve_01/v1_maze2`)
    pub level: String,
    /// pre-warm list ("class<TAB>name" lines), optional
    pub prewarm: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaemonCfg {
    pub socket: PathBuf,
    /// survarium.exe on this machine, and the Windows path the exe is told it has
    pub exe: PathBuf,
    pub exe_win: String,
    /// per-project output (logs, the private drive) under here
    pub out: PathBuf,
    pub max_matches: u32,
    pub idle_secs: u64,
    pub backstop_min: u32,
    /// a child exits when no frame arrived this long (the relay sends one DF per tick)
    pub read_timeout_secs: u64,
    pub tick_frames: bool,
    /// pre-warm lists: a project without its own `prewarm` uses `<prewarm_dir>/<project>.tsv` when that file exists and
    /// is not smaller than the largest list of a variant of its level and mode, else that variant's list
    /// ([`DaemonCfg::prewarm_pick`]); every match of such a
    /// project grows `<prewarm_dir>/<project>.tsv` by its own requests (the union, [`grow_prewarm_list`])
    pub prewarm_dir: Option<PathBuf>,
    /// projects whose zygote starts with the front (the others start on their first match)
    pub prestart: Vec<String>,
    /// zygotes at most, an evicted one that has not exited yet included (0 = no cap): a new map evicts idle ones, least
    /// recently used first, prestart ones last, never a busy one (`plan`)
    pub max_zygotes: u32,
    /// the game tables the projects come from when the config names none (`srp`'s `data_dir`); None = the tables
    /// compiled in
    pub data_dir: Option<PathBuf>,
    pub projects: Vec<ProjectCfg>,
}

impl DaemonCfg {
    /// The config file: `key = value` lines, `#` comments, and optionally one `[project NAME]` section per level project
    /// with `level = ...` and optionally `prewarm = ...`. Without any project section the daemon serves the projects of
    /// the game data ([`game_data::GameData::host_projects`]: every PvP row whose level ships in the client and the PvE
    /// mazes), so the list lives in one place; the lobby asks the daemon for it (`LIST_PROJECTS`).
    pub fn parse(text: &str) -> Result<DaemonCfg, String> {
        let mut c = DaemonCfg {
            socket: PathBuf::from("/run/srp/world-host.sock"),
            exe: PathBuf::new(),
            exe_win: String::new(),
            out: PathBuf::from("/var/lib/srp-world-host"),
            max_matches: 8,
            idle_secs: 1800,
            backstop_min: 65,
            read_timeout_secs: 120,
            tick_frames: true,
            prewarm_dir: None,
            prestart: Vec::new(),
            max_zygotes: 0,
            data_dir: None,
            projects: Vec::new(),
        };
        let mut cur: Option<ProjectCfg> = None;
        for (i, raw) in text.lines().enumerate() {
            let l = raw.split('#').next().unwrap_or("").trim();
            if l.is_empty() {
                continue;
            }
            let err = |m: &str| format!("line {}: {m}: {raw}", i + 1);
            if let Some(h) = l.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
                let name = h.strip_prefix("project").map(str::trim).filter(|n| !n.is_empty()).ok_or_else(|| err("expected [project NAME]"))?;
                if let Some(p) = cur.take() {
                    c.projects.push(p);
                }
                cur = Some(ProjectCfg { name: name.to_string(), level: String::new(), prewarm: None });
                continue;
            }
            let (k, v) = l.split_once('=').map(|(k, v)| (k.trim(), v.trim())).ok_or_else(|| err("expected key = value"))?;
            let num = |v: &str| v.parse::<u64>().map_err(|_| err("not a number"));
            match (&mut cur, k) {
                (Some(p), "level") => p.level = v.to_string(),
                (Some(p), "prewarm") => p.prewarm = Some(PathBuf::from(v)),
                (Some(_), _) => return Err(err("unknown project key")),
                (None, "socket") => c.socket = PathBuf::from(v),
                (None, "exe") => c.exe = PathBuf::from(v),
                (None, "exe_win") => c.exe_win = v.to_string(),
                (None, "out") => c.out = PathBuf::from(v),
                (None, "max_matches") => c.max_matches = num(v)? as u32,
                (None, "max_zygotes") => c.max_zygotes = num(v)? as u32,
                (None, "idle_secs") => c.idle_secs = num(v)?,
                (None, "backstop_min") => c.backstop_min = num(v)? as u32,
                (None, "read_timeout_secs") => c.read_timeout_secs = num(v)?,
                (None, "tick_frames") => c.tick_frames = num(v)? != 0,
                (None, "prewarm_dir") => c.prewarm_dir = Some(PathBuf::from(v)),
                (None, "data_dir") => c.data_dir = Some(PathBuf::from(v)),
                (None, "prestart") => c.prestart = v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                (None, _) => return Err(err("unknown key")),
            }
        }
        if let Some(p) = cur.take() {
            c.projects.push(p);
        }
        if c.projects.is_empty() {
            let d = c.data_dir.as_ref().ok_or("no [project] sections and no data_dir: set data_dir to the folder of the game tables (srp passes it)")?;
            let gd = game_data::GameData::from_dir(d).map_err(|e| format!("game data: {e}"))?;
            c.projects = gd.host_projects().into_iter().map(|(name, level)| ProjectCfg { name, level, prewarm: None }).collect();
        }
        if c.exe.as_os_str().is_empty() || c.exe_win.is_empty() {
            return Err("exe and exe_win are required".into());
        }
        if let Some(p) = c.projects.iter().find(|p| p.level.is_empty()) {
            return Err(format!("project {}: no level", p.name));
        }
        if let Some(x) = c.prestart.iter().find(|x| c.project(x).is_none()) {
            return Err(format!("prestart: unknown project {x}"));
        }
        Ok(c)
    }

    pub fn project(&self, name: &str) -> Option<&ProjectCfg> {
        self.projects.iter().find(|p| p.name == name)
    }

    /// The list a zygote of `p` loads ([`DaemonCfg::prewarm_pick`]).
    pub fn prewarm_of(&self, p: &ProjectCfg) -> Option<PathBuf> {
        self.prewarm_pick(p).list
    }

    /// A project's own list: its `prewarm` file, else `<prewarm_dir>/<name>.tsv` when that file exists.
    fn own_list(&self, p: &ProjectCfg) -> Option<PathBuf> {
        p.prewarm.clone().or_else(|| self.prewarm_dir.as_ref().map(|d| d.join(format!("{}.tsv", p.name))).filter(|f| f.is_file()))
    }

    /// Which pre-warm list a zygote of `p` loads, and where its matches record theirs:
    /// * a configured `prewarm` file, as it is (even when unreadable, so the error shows);
    /// * else its own `prewarm_dir` list when it has entries and at least as many as the best variant's list below (the
    ///   front merges the best variant's list into it before every zygote start, [`DaemonCfg::seed_prewarm`], so this
    ///   holds unless that merge failed);
    /// * else the list with the most entries (then the first name) among the projects of the same variant family
    ///   ([`variant_family`]: the name without a trailing `_v<digits>`) and level folder, e.g. `level_02_day_br_v3`
    ///   for `level_02_day_br_v2`. Its entries are resources of the same kinds the own list collects (players' kits,
    ///   the mode's items); the zygote queries them exactly as it queries an own list (`run::prewarm`);
    /// * else none: every match loads its shared resources itself.
    ///
    /// `record` = `<prewarm_dir>/<name>.tsv` unless the project has a configured `prewarm` file: every match grows it.
    pub fn prewarm_pick(&self, p: &ProjectCfg) -> PrewarmPick {
        let record = if p.prewarm.is_some() { None } else { self.prewarm_dir.as_ref().map(|d| d.join(format!("{}.tsv", p.name))) };
        let own = self.own_list(p).map(|l| (count_entries(&l), l));
        if let Some((entries, l)) = own.as_ref().filter(|_| p.prewarm.is_some()) {
            return PrewarmPick { source: PrewarmSource::Own, list: Some(l.clone()), entries: *entries, record };
        }
        match (own.filter(|o| o.0 > 0), self.best_sibling(p)) {
            (Some((entries, l)), b) if b.as_ref().is_none_or(|s| entries >= s.0) => PrewarmPick { source: PrewarmSource::Own, list: Some(l), entries, record },
            (_, Some((entries, name, l))) => PrewarmPick { source: PrewarmSource::Sibling(name.to_string()), list: Some(l), entries, record },
            (_, None) => PrewarmPick { source: PrewarmSource::None, list: None, entries: 0, record },
        }
    }

    /// The list with the most entries (then the first name) among the variants of `p` ([`is_variant`]): (entries,
    /// project, path).
    fn best_sibling(&self, p: &ProjectCfg) -> Option<(usize, &str, PathBuf)> {
        let mut sib: Vec<(usize, &str, PathBuf)> = self
            .projects
            .iter()
            .filter(|q| q.name != p.name && is_variant(q, p))
            .filter_map(|q| {
                let l = self.own_list(q)?;
                let n = count_entries(&l);
                (n > 0).then_some((n, q.name.as_str(), l))
            })
            .collect();
        sib.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
        sib.into_iter().next()
    }

    /// Before a zygote of `p` starts: its own `prewarm_dir` list takes in the best variant's list (the union,
    /// [`grow_prewarm_list`]), so a project whose own list only began, or whose variant is played more, never loads
    /// less than that variant's list (the entries are resources of the same kinds; a variant's entries leave every
    /// match byte unchanged, docs/notes/config.md). Nothing for a configured `prewarm` file or without `prewarm_dir`.
    /// The log line when entries were taken in or the merge failed.
    pub fn seed_prewarm(&self, p: &ProjectCfg) -> Option<String> {
        if p.prewarm.is_some() {
            return None;
        }
        let own = self.prewarm_dir.as_ref()?.join(format!("{}.tsv", p.name));
        let (_, name, l) = self.best_sibling(p)?;
        match std::fs::read_to_string(&l).and_then(|t| grow_prewarm_list(&own, &t)) {
            Ok((_, 0)) => None,
            Ok((n, added)) => Some(format!("front: pre-warm list {} took in variant {name}'s list: {n} entries, {added} new", own.display())),
            Err(e) => Some(format!("front: pre-warm list {}: variant {name}'s list not taken in: {e}", own.display())),
        }
    }

    /// The entries common to every list of the other projects of `p`'s kind ([`project_kind`]: PvP maps or PvE
    /// missions), in the order of the first such project's list: (lists, entries as list text). None with fewer than
    /// [`MIN_COMMON_LISTS`] non-empty lists (one list is one map's record, with its mode's own items).
    pub fn common_seed(&self, p: &ProjectCfg) -> Option<(usize, String)> {
        let lists: Vec<Vec<(u32, String)>> = self
            .projects
            .iter()
            .filter(|q| q.name != p.name && project_kind(q) == project_kind(p))
            .filter_map(|q| std::fs::read_to_string(self.own_list(q)?).ok())
            .map(|t| list_entries(&t))
            .filter(|e| !e.is_empty())
            .collect();
        if lists.len() < MIN_COMMON_LISTS {
            return None;
        }
        let sets: Vec<std::collections::HashSet<&(u32, String)>> = lists[1..].iter().map(|l| l.iter().collect()).collect();
        let mut seen = std::collections::HashSet::new();
        let mut out = String::new();
        for e in &lists[0] {
            if sets.iter().all(|s| s.contains(e)) && seen.insert(e) {
                out.push_str(&format!("{:#x}\t{}\n", e.0, e.1));
            }
        }
        Some((lists.len(), out))
    }

    /// Before a zygote of `p` starts, after [`DaemonCfg::seed_prewarm`]: a project with neither an own list nor a
    /// variant's list takes in the common seed ([`DaemonCfg::common_seed`]) as its own `prewarm_dir` list, which its
    /// matches then grow as any own list. The seed's entries are queried as a variant's list is (`run::prewarm`, after
    /// the project's own level): an entry a match also asks for is cooked in the state an own list would cook it in,
    /// one it never asks for only stays in the resource cache (docs/notes/config.md 1.1). Returns the log line and,
    /// when the list was seeded now, the number of lists the seed came from. Nothing for a configured `prewarm` file or
    /// without `prewarm_dir`.
    pub fn seed_common(&self, p: &ProjectCfg) -> Option<(String, Option<usize>)> {
        if p.prewarm.is_some() {
            return None;
        }
        let own = self.prewarm_dir.as_ref()?.join(format!("{}.tsv", p.name));
        if count_entries(&own) > 0 || self.best_sibling(p).is_some() {
            return None;
        }
        let kind = project_kind(p);
        let Some((n, text)) = self.common_seed(p) else {
            return Some((format!("front: pre-warm list {}: no list, no variant's list, no common seed (fewer than {MIN_COMMON_LISTS} {kind} lists)", own.display()), None));
        };
        let k = list_entries(&text).len();
        if k == 0 {
            return Some((format!("front: pre-warm list {}: the {n} {kind} lists have no entry in common: no common seed", own.display()), None));
        }
        match grow_prewarm_list(&own, &text) {
            Ok((entries, added)) => Some((format!("front: pre-warm list {}: common seed of {n} {kind} lists, {k} entries ({entries} in the list, {added} new)", own.display()), Some(n))),
            Err(e) => Some((format!("front: pre-warm list {}: common seed of {n} {kind} lists not written: {e}", own.display()), None)),
        }
    }

    /// The base pre-warm set's exclusion file (`0xCLASS<TAB>name` lines that never join the set): grown by a zygote
    /// whose level load found an entry the set added (`baseset::check_end`). In `prewarm_dir`, else in `out`.
    pub fn base_exclude_path(&self) -> PathBuf {
        self.prewarm_dir.as_ref().unwrap_or(&self.out).join("base-exclude.tsv")
    }

    /// The base pre-warm set (`baseset::base_set` on every served project's own list, each file once, in the projects'
    /// order; the common seed is written as an own list, so it counts as one). Returns the set, the lists it came from
    /// and the entries the exclusion file took out.
    pub fn base_prewarm_set(&self) -> (Vec<(u32, String)>, usize, usize) {
        let mut seen = std::collections::HashSet::new();
        let lists: Vec<Vec<(u32, String)>> = self
            .projects
            .iter()
            .filter_map(|p| self.own_list(p))
            .filter(|l| seen.insert(l.clone()))
            .filter_map(|l| std::fs::read_to_string(l).ok())
            .map(|t| list_entries(&t))
            .filter(|e| !e.is_empty())
            .collect();
        let exclude = std::fs::read_to_string(self.base_exclude_path()).map(|t| list_entries(&t).into_iter().collect()).unwrap_or_default();
        let (set, excluded) = crate::baseset::base_set(&lists, self.max_zygotes, &exclude);
        (set, lists.len(), excluded)
    }
}

/// Lists a common seed needs at least ([`DaemonCfg::common_seed`]).
pub const MIN_COMMON_LISTS: usize = 2;

/// The kind a project's lists are compared within for the common seed: "PvE" for a mission (level folder `pve_...`),
/// else "PvP".
pub fn project_kind(p: &ProjectCfg) -> &'static str {
    if p.level.starts_with("pve_") { "PvE" } else { "PvP" }
}

/// Diagnosis: `HOST_NO_COMMON_SEED=1` in the front's environment leaves projects without a list unseeded (A/B runs).
fn common_seed_off() -> bool {
    std::env::var("HOST_NO_COMMON_SEED").is_ok_and(|v| v == "1")
}

/// Whose pre-warm list a zygote loads (logged by the front, the zygote and every match).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrewarmSource {
    Own,
    /// a variant of the same level and mode (its project name)
    Sibling(String),
    /// the own list, seeded just now from the entries common to this many lists of its kind
    Common(usize),
    None,
}

impl std::fmt::Display for PrewarmSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrewarmSource::Own => write!(f, "own"),
            PrewarmSource::Sibling(n) => write!(f, "sibling {n}"),
            PrewarmSource::Common(n) => write!(f, "common seed of {n} lists"),
            PrewarmSource::None => write!(f, "none"),
        }
    }
}

/// [`DaemonCfg::prewarm_pick`]: the list to load (and its entry count), and the project's own list its matches grow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrewarmPick {
    pub source: PrewarmSource,
    pub list: Option<PathBuf>,
    pub entries: usize,
    pub record: Option<PathBuf>,
}

/// The variant family of a project: its name without a trailing `_v<digits>` (`level_02_day_br_v3` ->
/// `level_02_day_br`); any other name is its own family (`level_02_day_br`, `pve_01_v1_maze1`, `level_02_day_slr_vn`).
pub fn variant_family(name: &str) -> &str {
    if let Some(i) = name.rfind("_v") {
        let tail = &name[i + 2..];
        if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
            return &name[..i];
        }
    }
    name
}

/// Two projects are variants of one level and mode: same variant family and the same level folder (`level_02/...`).
fn is_variant(a: &ProjectCfg, b: &ProjectCfg) -> bool {
    let folder = |p: &ProjectCfg| p.level.split('/').next().unwrap_or("").to_string();
    variant_family(&a.name) == variant_family(&b.name) && folder(a) == folder(b)
}

/// The (class, name) entries of a pre-warm list ("0xCLASS<TAB>name" lines, read as `run::prewarm` reads them); other
/// lines are skipped.
pub fn list_entries(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|l| {
            let (c, n) = l.split_once('\t')?;
            let c = u32::from_str_radix(c.trim_start_matches("0x"), 16).ok()?;
            (!n.is_empty()).then(|| (c, n.to_string()))
        })
        .collect()
}

/// Entries of the list file at `path` (0 when unreadable).
fn count_entries(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path).map(|t| list_entries(&t).len()).unwrap_or(0)
}

/// The union of a project's pre-warm list `old` and a match creation's list `new`: every entry of `old` in its order,
/// then the entries of `new` that `old` lacks, as "0xCLASS<TAB>name" lines (each (class, name) once). Returns
/// (text, entries, added).
pub fn prewarm_union(old: &str, new: &str) -> (String, usize, usize) {
    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    let mut kept = 0;
    let mut added = 0;
    for (i, text) in [old, new].into_iter().enumerate() {
        for (c, n) in list_entries(text) {
            if seen.insert((c, n.clone())) {
                out.push_str(&format!("{c:#x}\t{n}\n"));
                if i == 0 {
                    kept += 1;
                } else {
                    added += 1;
                }
            }
        }
    }
    (out, kept + added, added)
}

/// Grow the pre-warm list at `path` by a match creation's list `new` ([`prewarm_union`]), under an exclusive `flock` on
/// `<path>.lock` (the matches of one project may end their creations at the same moment). The file is replaced
/// through a temporary name and a rename, so a zygote that starts meanwhile reads the old or the new list, never a part;
/// nothing is written when nothing is new (no empty list is ever made). Returns (entries, added).
pub fn grow_prewarm_list(path: &std::path::Path, new: &str) -> io::Result<(usize, usize)> {
    let with = |suffix: &str| {
        let mut s = path.as_os_str().to_owned();
        s.push(suffix);
        PathBuf::from(s)
    };
    let lock = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(with(".lock"))?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let old = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let (text, entries, added) = prewarm_union(&old, new);
    if added > 0 {
        let tmp = with(&format!(".{}.tmp", std::process::id()));
        if let Err(e) = std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, path)) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }
    // the lock goes with `lock`
    Ok((entries, added))
}

/// The classes a zygote can pre-warm by name alone: configs 1F, animations 31, 2C, 3, E, F, 13, 14, D4,
/// 8B. Item, weapon-module and combined-model classes (43, 48, 49, 50, 10, 15, 16) need a match's user data (class
/// 0x43 without it faults in its cook at 0x20033516E) and stay with the children.
pub const PREWARM_CLASSES: [u32; 10] = [0x1F, 0x31, 0x2C, 0x3, 0xE, 0xF, 0x13, 0x14, 0xD4, 0x8B];

/// A pre-warm list from a match creation's query log (`HOST_QUERY_LOG=1` writes `queries.tsv`: header, then
/// `class<TAB>name<TAB>tid<TAB>caller`): the unique (class, name) pairs of `classes`, in first-query order, as
/// `0xCLASS<TAB>name` lines. Names of 119+ bytes were cut by the log's 120-byte buffer and are dropped.
pub fn prewarm_list(queries: &str, classes: &[u32]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out = String::new();
    for l in queries.lines().skip(1) {
        let mut f = l.split('\t');
        let (Some(c), Some(n)) = (f.next(), f.next()) else { continue };
        let Ok(c) = u32::from_str_radix(c.trim_start_matches("0x"), 16) else { continue };
        if n.is_empty() || n.len() >= 119 || !classes.contains(&c) || !seen.insert((c, n.to_string())) {
            continue;
        }
        out.push_str(&format!("{c:#x}\t{n}\n"));
    }
    out
}

/// `world-host prewarm-list QUERIES.tsv OUT.tsv`: write a project's pre-warm list from a match's query log.
pub fn prewarm_list_main(args: &[String]) -> i32 {
    let (Some(q), Some(o)) = (args.first(), args.get(1)) else {
        eprintln!("usage: world-host prewarm-list QUERIES.tsv OUT.tsv");
        return 2;
    };
    let text = match std::fs::read_to_string(q) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{q}: {e}");
            return 1;
        }
    };
    let list = prewarm_list(&text, &PREWARM_CLASSES);
    if let Err(e) = std::fs::write(o, &list) {
        eprintln!("{o}: {e}");
        return 1;
    }
    println!("{o}: {} requests", list.lines().count());
    0
}

fn fault_frame(reason: &str) -> Vec<u8> {
    FromHost::Fault { reason: reason.chars().take(world_host_proto::MAX_TEXT).collect() }.frame().unwrap_or_default()
}

// ---------------------------------------------------------------------------------------------------------------------
// front

struct Zygote {
    /// its pid: a child of the base process, not of the front
    pid: u32,
    /// a pidfd of it (from the base process): readable once it ended, and the way to kill it (no pid reuse)
    pidfd: OwnedFd,
    ctl: UnixStream,
    /// the last match handed to it (its start for a new one)
    last_used: Instant,
    /// hand-overs sent to it
    sent: u32,
    /// its latest status record (fdpass::send_status): hand-overs taken, matches running
    status: (u32, u32),
    /// the hand-overs of the last RECENT_FORK (the room status counts them as fresh forks, as the zygote does)
    handed: Vec<Instant>,
    /// when it was loaded (the room status reports its age: the lobby drains the oldest map under a full limit)
    loaded: Instant,
    /// a cut status record
    rx: Vec<u8>,
}

impl Zygote {
    /// Read the status records that arrived (never blocks).
    fn poll_status(&mut self) {
        if let Ok(Some(s)) = crate::fdpass::read_status(&self.ctl, &mut self.rx) {
            self.status = s;
        }
    }

    /// It took every hand-over and runs no match: evicting it frees its memory at once.
    fn idle(&self) -> bool {
        self.status == (self.sent, 0)
    }
}

/// One loaded map ("zygote") as the memory limit sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ZyView {
    project: String,
    /// its control socket is open: the front can hand matches to it (false = evicted, it is exiting)
    live: bool,
    /// it took every hand-over and runs no match
    idle: bool,
    /// a `prestart` map
    prestart: bool,
    /// the last match handed to it (its start for a new one)
    last_used: Instant,
}

/// What a match for a project needs under `max_zygotes`.
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    /// its map is loaded: hand the match over
    HandOver,
    /// room for its map: load it
    Load,
    /// every loaded map runs matches, but the machine has this much memory available (MB): load it one over the limit
    LoadOver(u64),
    /// evict these idle maps first (then load once they exited)
    Evict(Vec<String>),
    /// no room yet: wait (why)
    Wait(&'static str),
}

/// The memory limit (`cap` = max_zygotes, 0 = no limit) for a match of `project`, given every zygote that still exists
/// (live ones and evicted ones that have not exited: both hold their memory). A map never gets a second zygote while one
/// exists. Only an idle zygote is evicted (a busy one would free nothing until its matches end, and its matches share
/// its pages): the least recently used first, `prestart` maps after every other one. With no idle zygote to evict the
/// idle maps over the limit are evicted down to it (memory or not), and once no evicted map is still exiting the map
/// loads one over the limit while `free_mb` (MemAvailable, read only then) is at least `OVER_CAP_FREE_MB`; else the
/// match waits for room.
fn plan(project: &str, cap: u32, zy: &[ZyView], free_mb: impl FnOnce() -> Option<u64>) -> Plan {
    if zy.iter().any(|z| z.project == project && z.live) {
        return Plan::HandOver;
    }
    if zy.iter().any(|z| z.project == project) {
        return Plan::Wait("its evicted zygote has not exited yet");
    }
    plan_new(cap, zy, free_mb)
}

/// [`plan`] for a map that has no zygote (neither live nor exiting): load, evict, load one over the limit, or wait.
fn plan_new(cap: u32, zy: &[ZyView], free_mb: impl FnOnce() -> Option<u64>) -> Plan {
    if cap == 0 || zy.len() < cap as usize {
        return Plan::Load;
    }
    let live = zy.iter().filter(|z| z.live).count();
    if live < cap as usize {
        return Plan::Wait("an evicted map is still exiting");
    }
    let need = live + 1 - cap as usize;
    let mut idle: Vec<&ZyView> = zy.iter().filter(|z| z.live && z.idle).collect();
    idle.sort_by_key(|z| (z.prestart, z.last_used));
    if idle.len() < need {
        // Here live >= cap and idle < live + 1 - cap. When the busy maps fit in the limit, the idle maps over it go
        // first (`down` = live - cap of them, the oldest first), memory or not: back to the limit. Then, at the limit
        // with every map busy and none still exiting (an exiting map holds its memory and counts), one map over the
        // limit, never more, while the memory is there: the players of this match have loaded already, a refusal
        // sends them back
        let down = live - cap as usize;
        if down > 0 && idle.len() >= down {
            return Plan::Evict(idle.into_iter().take(down).map(|z| z.project.clone()).collect());
        }
        if down == 0 && zy.len() > live {
            return Plan::Wait("an evicted map is still exiting");
        }
        if let Some(mb) = (down == 0).then(free_mb).flatten().filter(|&mb| mb >= OVER_CAP_FREE_MB) {
            return Plan::LoadOver(mb);
        }
        return Plan::Wait("the busy maps fill the limit");
    }
    Plan::Evict(idle.into_iter().take(need).map(|z| z.project.clone()).collect())
}

/// How long a match waits for room under max_zygotes (a map finishing its matches, an evicted one exiting) before it is
/// refused. The relay gives the host 45 s for `Ready` (`host_start_timeout`); a zygote forked from a ready base loads
/// its level and list in about 1-3 s of it (measured: PvP 1.0 s, PvE 3.1 s), a base start takes about 3 s more.
const ROOM_WAIT: Duration = Duration::from_secs(20);

/// MemAvailable (MB) that lets a map load one over max_zygotes while every loaded map runs matches: one map
/// ([`ZYGOTE_MB`]: the largest measures 0.32-0.34 GB, a PvE mission 0.25-0.26 GB) plus 1 GB for the busy maps'
/// matches to grow (a match measured up to 0.14 GB).
const OVER_CAP_FREE_MB: u64 = ZYGOTE_MB + 1000;

/// MemAvailable (MB) that lets a zygote take a match over max_matches (at most twice it): one match (an unwarmed one up
/// to about 0.16 GB, recorded) plus 1 GB for the running matches to grow. Its players have loaded already: a refusal
/// sends them back.
const OVER_MATCHES_FREE_MB: u64 = 1200;

/// One match's memory (MB, an unwarmed child, recorded 127-158 MB) and a time after which a fresh child's creation
/// has shown it in MemAvailable (creations were recorded at about 0.5-1 s; 5 s is a margin, not a measurement): every
/// fork of the last `RECENT_FORK` keeps one `MATCH_MB` more free, so a burst of hand-overs does not read the same
/// MemAvailable over and over.
const MATCH_MB: u64 = 160;
const RECENT_FORK: Duration = Duration::from_secs(5);

/// Whether a zygote running `running` matches (`recent` of them forked in the last `RECENT_FORK`) takes one more under
/// `max` (max_matches): `Some(None)` within the limit, `Some(Some(mb))` over it with `free_mb` (MemAvailable, read only
/// then) at least `OVER_MATCHES_FREE_MB` + `recent` x `MATCH_MB` and fewer than twice the limit running, `None` refused.
fn match_room(running: u32, recent: u32, max: u32, free_mb: impl FnOnce() -> Option<u64>) -> Option<Option<u64>> {
    if running < max {
        return Some(None);
    }
    if running >= max.saturating_mul(2) {
        return None;
    }
    let need = OVER_MATCHES_FREE_MB + MATCH_MB * u64::from(recent);
    free_mb().filter(|&mb| mb >= need).map(Some)
}

/// One loaded map's size (MB) for the room status and the over-the-limit rule: a map that the status counts as loading
/// takes this much of the memory the next one-over-the-limit load would read. An upper end with a margin: the largest
/// map measures 0.32-0.34 GB, a PvE mission 0.25-0.26 GB.
const ZYGOTE_MB: u64 = 420;

/// Creates the front has read and not yet handed over or refused, per project (a `route` that waits for room, loads its
/// map or hands it over): the room status counts them, so a status read right after a match's Create shows it.
static ARRIVING: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());

/// One Create in [`ARRIVING`] while it lives.
struct Arriving(String);

impl Arriving {
    fn new(project: &str) -> Arriving {
        let mut a = ARRIVING.lock().unwrap();
        match a.iter_mut().find(|(p, _)| p == project) {
            Some((_, n)) => *n += 1,
            None => a.push((project.to_string(), 1)),
        }
        Arriving(project.to_string())
    }
}

impl Drop for Arriving {
    fn drop(&mut self) {
        let mut a = ARRIVING.lock().unwrap();
        if let Some(i) = a.iter().position(|(p, _)| *p == self.0) {
            a[i].1 -= 1;
            if a[i].1 == 0 {
                a.swap_remove(i);
            }
        }
    }
}

/// A live zygote's matches for the room status.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ZyLoad {
    project: String,
    running: u32,
    /// handed over and not taken yet, plus Creates arriving for it
    starting: u32,
    /// hand-overs of the last RECENT_FORK plus Creates arriving for it (each a fresh fork soon)
    recent: u32,
    /// seconds since it was loaded
    loaded_secs: u32,
}

/// The room status (`Status` 0x8C) from what the front sees: `zy` every zygote that exists (`views`), `loads` the live
/// ones' matches, `arriving` the Creates on their way per project, `free_mb` MemAvailable. The rules are the ones a
/// match meets: [`match_room`] per loaded map (its running and starting matches); an idle map's rank in `plan`'s
/// eviction order (least recently used first, `prestart` maps last); and for maps that are not loaded, [`plan_new`]
/// again and again, each new map counted as a busy zygote, an evicted map as gone (the evictions counted for the new map
/// that follows them), a Create arriving for a map without a zygote as one loading and one arriving for an idle map as
/// keeping it busy, until it says wait.
fn room_status(cap: u32, max_matches: u32, base_ready: bool, zy: &[ZyView], loads: &[ZyLoad], arriving: &[(String, u32)], free_mb: Option<u64>) -> world_host_proto::status::HostStatus {
    use world_host_proto::status::{HostStatus, LoadKind, LoadedMap, MatchRoom, NewMap, NewMaps, MAX_NEW_MAPS};
    let n16 = |n: u32| u16::try_from(n).unwrap_or(u16::MAX);
    let on_way = |p: &str| arriving.iter().any(|(a, n)| a == p && *n > 0);
    // a Create on its way to an idle map is handed to it next: the map counts busy
    let mut v: Vec<ZyView> = zy.iter().map(|z| ZyView { idle: z.idle && !on_way(&z.project), ..z.clone() }).collect();
    let mut idle: Vec<&ZyView> = v.iter().filter(|z| z.live && z.idle).collect();
    idle.sort_by_key(|z| (z.prestart, z.last_used));
    let rank = |p: &str| idle.iter().position(|z| z.project == p).map(|i| n16(i as u32));
    let loaded = loads
        .iter()
        .map(|l| LoadedMap {
            project: l.project.clone(),
            running: n16(l.running),
            starting: n16(l.starting),
            room: match match_room(l.running.saturating_add(l.starting), l.recent, max_matches, || free_mb) {
                Some(None) => MatchRoom::Within,
                Some(Some(_)) => MatchRoom::Over,
                None => MatchRoom::Full,
            },
            evict: rank(&l.project),
            loaded_secs: l.loaded_secs,
        })
        .collect();
    let new_maps = if !base_ready {
        NewMaps::BaseNotReady
    } else {
        let now = Instant::now();
        let busy = |p: &str| ZyView { project: p.to_string(), live: true, idle: false, prestart: false, last_used: now };
        for (p, _) in arriving.iter().filter(|(p, n)| *n > 0 && !zy.iter().any(|z| z.project == *p)) {
            v.push(busy(p));
        }
        let mut free = free_mb;
        let mut maps: Vec<NewMap> = Vec::new();
        let mut evicts = 0u16;
        while maps.len() < MAX_NEW_MAPS {
            let kind = match plan_new(cap, &v, || free) {
                Plan::Load => LoadKind::Within,
                Plan::LoadOver(_) => LoadKind::Over,
                // evicted maps are gone; the plan after them says what the new map gets
                Plan::Evict(names) => {
                    v.retain(|z| !names.contains(&z.project));
                    evicts = evicts.saturating_add(n16(names.len() as u32));
                    continue;
                }
                Plan::Wait(_) | Plan::HandOver => break,
            };
            maps.push(NewMap { kind, evicts });
            evicts = 0;
            // a name no project has (project names have no spaces)
            v.push(busy(&format!("new map {}", maps.len())));
            free = free.map(|mb| mb.saturating_sub(ZYGOTE_MB));
        }
        if maps.is_empty() { NewMaps::NoRoom } else { NewMaps::Room(maps) }
    };
    HostStatus {
        max_zygotes: (cap != 0).then_some(cap),
        max_matches,
        free_mb: free_mb.map(|mb| u32::try_from(mb).unwrap_or(u32::MAX)),
        new_maps,
        loaded,
        drain: None,
    }
}

/// How long one map may drain. A PvP match ends within about 15-20 min and a PvE mission within 45 min, and the queues
/// start nothing new on the drained map while another fits, so its matches normally end well within this. A map still
/// busy after a whole turn keeps getting matches the queues cannot steer away (custom rooms, clan battles, a PvE maze
/// whose other maze has no room): the next map gets its turn.
const DRAIN_TURN: Duration = Duration::from_secs(20 * 60);

/// The front's drain choice: while no new map can load, one busy map at a time gets no new queue match while another
/// fits, so that it turns idle, can be evicted, and the queues' rotation loads its next map.
#[derive(Debug, Default)]
struct DrainTurn {
    /// the map being drained and since when
    current: Option<(String, Instant)>,
    /// maps that stayed busy for a whole DRAIN_TURN: skipped until a drain succeeds, no other map is left, or room comes
    stuck: Vec<String>,
}

static DRAIN: Mutex<DrainTurn> = Mutex::new(DrainTurn { current: None, stuck: Vec::new() });

impl DrainTurn {
    /// The map to drain at `now`. `no_room` = no new map can load while the base is ready (with room the rotation loads
    /// new maps by itself; without a ready base nothing loads); `cands` = the busy maps that are not `prestart` maps
    /// (those stay loaded, as eviction takes them last), the longest loaded first. The rule: keep the current map while
    /// it is busy, for at most DRAIN_TURN; once it is idle (or gone) it drained: the next one, the longest loaded; a map
    /// still busy after its turn is passed over until a drain succeeds; when every candidate was passed over, start
    /// again with the longest loaded.
    fn turn(&mut self, no_room: bool, cands: &[&str], now: Instant) -> Option<String> {
        if !no_room {
            self.current = None;
            self.stuck.clear();
            return None;
        }
        if let Some((p, since)) = self.current.take() {
            if cands.contains(&p.as_str()) {
                if now.saturating_duration_since(since) < DRAIN_TURN {
                    self.current = Some((p.clone(), since));
                    return Some(p);
                }
                self.stuck.push(p);
            } else {
                self.stuck.clear();
            }
        }
        let next = match cands.iter().find(|c| !self.stuck.iter().any(|s| s == *c)) {
            Some(c) => *c,
            None => {
                self.stuck.clear();
                *cands.first()?
            }
        };
        self.current = Some((next.to_string(), now));
        Some(next.to_string())
    }
}

/// The room status now (the `ListStatus` answer): the base's state, every zygote's matches (their status records read
/// first), the Creates arriving and MemAvailable. BASE is read alone, before the zygotes lock; under the zygotes lock
/// RETIRED (`views`) and ARRIVING are taken, the order `route` keeps too.
fn host_status(cfg: &DaemonCfg, zygotes: &Mutex<HashMap<String, Zygote>>) -> world_host_proto::status::HostStatus {
    let base_ready = BASE.lock().unwrap().as_ref().is_some_and(|b| b.ready);
    reap_retired();
    let free = available_mb();
    let mut z = zygotes.lock().unwrap();
    drop_exited(&mut z);
    let v = views(cfg, &mut z);
    let arriving = ARRIVING.lock().unwrap().clone();
    let now = Instant::now();
    let on_way = |p: &str| arriving.iter().find(|(a, _)| a == p).map_or(0, |a| a.1);
    let loads: Vec<ZyLoad> = z
        .iter_mut()
        .map(|(name, x)| {
            x.handed.retain(|t| now.saturating_duration_since(*t) < RECENT_FORK);
            ZyLoad {
                project: name.clone(),
                running: x.status.1,
                starting: x.sent.wrapping_sub(x.status.0).saturating_add(on_way(name)),
                recent: (x.handed.len() as u32).saturating_add(on_way(name)),
                loaded_secs: u32::try_from(now.saturating_duration_since(x.loaded).as_secs()).unwrap_or(u32::MAX),
            }
        })
        .collect();
    drop(z);
    let mut st = room_status(cfg.max_zygotes, cfg.max_matches, base_ready, &v, &loads, &arriving, free);
    // the drain: the busy maps that are not prestart maps, the longest loaded first
    let mut busy: Vec<&world_host_proto::status::LoadedMap> = st.loaded.iter().filter(|m| m.running > 0 || m.starting > 0).filter(|m| !cfg.prestart.contains(&m.project)).collect();
    busy.sort_by(|a, b| b.loaded_secs.cmp(&a.loaded_secs).then_with(|| a.project.cmp(&b.project)));
    let cands: Vec<&str> = busy.iter().map(|m| m.project.as_str()).collect();
    let no_room = st.new_maps == world_host_proto::status::NewMaps::NoRoom;
    st.drain = DRAIN.lock().unwrap().turn(no_room, &cands, now);
    st
}

/// MemAvailable of `/proc/meminfo` in MB.
fn available_mb() -> Option<u64> {
    let t = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = t.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Zygotes the front evicted (control socket closed): they hold their memory until they exited, then are reaped.
/// Each with its pidfd, the time it was evicted and whether it was killed.
static RETIRED: Mutex<Vec<Retired<OwnedFd>>> = Mutex::new(Vec::new());

/// An evicted zygote that still exists (`proc_`: what tells whether it ended and kills it; a pidfd in the front).
struct Retired<P> {
    name: String,
    pid: u32,
    proc_: P,
    since: Instant,
    killed: bool,
}

/// The process behind a pidfd has not ended (a pidfd turns readable when its process ends).
fn pidfd_alive(fd: &OwnedFd) -> bool {
    let mut p = libc::pollfd { fd: fd.as_raw_fd(), events: libc::POLLIN, revents: 0 };
    let r = unsafe { libc::poll(&mut p, 1, 0) };
    !pidfd_ended(r, p.revents)
}

/// A pidfd poll's answer (`r` = poll's result, `revents`): ended only on POLLIN or POLLHUP; a poll error (EINTR,
/// ENOMEM) or no event says nothing, so the process counts as alive.
fn pidfd_ended(r: i32, revents: i16) -> bool {
    r > 0 && revents & (libc::POLLIN | libc::POLLHUP) != 0
}

/// SIGKILL through a pidfd (it names exactly that process, even after its pid was reaped and reused).
fn pidfd_kill(fd: &OwnedFd) {
    unsafe { libc::syscall(libc::SYS_pidfd_send_signal, fd.as_raw_fd(), libc::SIGKILL, std::ptr::null::<libc::siginfo_t>(), 0) };
}

/// One reaper turn over the evicted zygotes at `now`: an ended one is dropped, one still alive RETIRE_KILL after its
/// eviction is killed once (and stays listed, and counted, until it ended). Returns the log lines.
fn retire_turn<P>(v: &mut Vec<Retired<P>>, now: Instant, alive: impl Fn(&P) -> bool, kill: impl Fn(&P)) -> Vec<String> {
    let mut lines = Vec::new();
    v.retain_mut(|r| {
        if !alive(&r.proc_) {
            lines.push(format!("front: evicted zygote {} (pid {}) exited (its status is in the base's log)", r.name, r.pid));
            return false;
        }
        let age = now.saturating_duration_since(r.since);
        if !r.killed && age >= RETIRE_KILL {
            lines.push(format!("front: evicted zygote {} (pid {}) still alive {} s after eviction: killed", r.name, r.pid, age.as_secs()));
            kill(&r.proc_);
            r.killed = true;
        }
        true
    });
    lines
}

/// An evicted zygote runs no match and exits at once when its control socket closes; one still alive this long
/// after eviction is hung and is killed, so it cannot hold a max_zygotes slot (and its memory) forever. Half of
/// ROOM_WAIT: a match waiting for that slot still gets it.
const RETIRE_KILL: Duration = Duration::from_secs(10);

/// Reap evicted zygotes that exited; the ones still exiting stay listed (and counted). One still alive RETIRE_KILL
/// after eviction is killed (SIGKILL) and reaped on a later turn.
fn reap_retired() {
    let lines = retire_turn(&mut RETIRED.lock().unwrap(), Instant::now(), pidfd_alive, pidfd_kill);
    for l in lines {
        hlog!("{l}");
    }
}

/// Every zygote that exists, for `plan`: the live ones (their status read first) and the evicted ones still exiting.
fn views(cfg: &DaemonCfg, z: &mut HashMap<String, Zygote>) -> Vec<ZyView> {
    // a Create on its way to an idle map is handed to it next: it counts busy (as in the room status), so no other route
    // evicts it meanwhile (zygotes -> ARRIVING, the order route keeps)
    let arriving = ARRIVING.lock().unwrap().clone();
    let on_way = |n: &str| arriving.iter().any(|(p, k)| p == n && *k > 0);
    let mut v: Vec<ZyView> = z
        .iter_mut()
        .map(|(n, x)| {
            x.poll_status();
            ZyView { project: n.clone(), live: true, idle: x.idle() && !on_way(n), prestart: cfg.prestart.contains(n), last_used: x.last_used }
        })
        .collect();
    let now = Instant::now();
    v.extend(RETIRED.lock().unwrap().iter().map(|r| ZyView { project: r.name.clone(), live: false, idle: false, prestart: cfg.prestart.contains(&r.name), last_used: now }));
    v
}

/// "N live (B busy) + E exiting of max_zygotes C" for the decision lines.
fn counts(v: &[ZyView], cap: u32) -> String {
    let live = v.iter().filter(|x| x.live).count();
    let busy = v.iter().filter(|x| x.live && !x.idle).count();
    let cap = if cap == 0 { "no limit".to_string() } else { cap.to_string() };
    format!("{live} live ({busy} busy) + {} exiting, max_zygotes {cap}", v.len() - live)
}

/// Drop the zygotes that exited by themselves (idle_secs, a crash).
fn drop_exited(z: &mut HashMap<String, Zygote>) {
    z.retain(|name, x| {
        let alive = pidfd_alive(&x.pidfd);
        if !alive {
            hlog!("front: zygote {name} (pid {}) exited (its status is in the base's log)", x.pid);
        }
        alive
    });
}

/// `world-host daemon --config FILE`.
pub fn front_main(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let Some(path) = get("--config") else {
        eprintln!("usage: world-host daemon --config FILE");
        return 2;
    };
    let cfg = match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| DaemonCfg::parse(&t)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{path}: {e}");
            return 2;
        }
    };
    let _ = std::fs::create_dir_all(&cfg.out);
    crate::log::open(cfg.out.join("front.log").to_str().unwrap_or("front.log"));
    let _ = std::fs::remove_file(&cfg.socket);
    let listener = match UnixListener::bind(&cfg.socket) {
        Ok(l) => l,
        Err(e) => {
            hlog!("front: cannot bind {}: {e}", cfg.socket.display());
            return 1;
        }
    };
    hlog!("front: listening on {} ({} project(s))", cfg.socket.display(), cfg.projects.len());
    let cfg = Arc::new(cfg);
    let zygotes: Arc<Mutex<HashMap<String, Zygote>>> = Arc::new(Mutex::new(HashMap::new()));
    // the base process starts now; every second: an evicted zygote is killed (RETIRE_KILL) and reaped on time also
    // while no match comes, and the base is checked (ready, ended, restarted)
    base_turn(&cfg);
    {
        let cfg = cfg.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                reap_retired();
                base_turn(&cfg);
            }
        });
    }
    // prestart maps load through the base once it is ready; the listener answers meanwhile
    {
        let (cfg, zygotes) = (cfg.clone(), zygotes.clone());
        std::thread::spawn(move || prestart(&cfg, &zygotes));
    }
    for conn in listener.incoming() {
        match conn {
            Ok(s) => {
                let (cfg, zy) = (cfg.clone(), zygotes.clone());
                std::thread::spawn(move || route(s, &cfg, &zy));
            }
            Err(e) => hlog!("front: accept: {e}"),
        }
    }
    0
}

/// Read the connection's `Create`, then hand it to its project's zygote.
fn route(mut s: UnixStream, cfg: &DaemonCfg, zygotes: &Mutex<HashMap<String, Zygote>>) {
    let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
    let frame = match read_frame(&mut s) {
        Ok(Some(f)) => f,
        Ok(None) => return,
        Err(e) => {
            hlog!("front: first frame: {e}");
            let _ = s.write_all(&fault_frame(&format!("first frame: {e}")));
            return;
        }
    };
    // the lobby's project-list query: answer with the served projects and close
    if frame.ty == world_host_proto::ty::LIST_PROJECTS {
        let names: Vec<String> = cfg.projects.iter().map(|p| p.name.clone()).collect();
        match world_host_proto::projects_payload(&names).and_then(|p| world_host_proto::frame_bytes(world_host_proto::ty::PROJECTS, &p)) {
            Ok(b) => {
                let _ = s.write_all(&b);
            }
            Err(e) => {
                let _ = s.write_all(&fault_frame(&format!("Projects: {e}")));
            }
        }
        return;
    }
    // the lobby's room status query (about every 0.5 s): answer and close
    if frame.ty == world_host_proto::ty::LIST_STATUS {
        let st = host_status(cfg, zygotes);
        match st.payload().and_then(|p| world_host_proto::frame_bytes(world_host_proto::ty::STATUS, &p)) {
            Ok(b) => {
                let _ = s.write_all(&b);
            }
            Err(e) => {
                let _ = s.write_all(&fault_frame(&format!("Status: {e}")));
            }
        }
        return;
    }
    let create = match ToHost::decode(&frame) {
        Ok(ToHost::Create(c)) => c,
        Ok(_) => {
            let _ = s.write_all(&fault_frame("the first frame must be Create"));
            return;
        }
        Err(e) => {
            let _ = s.write_all(&fault_frame(&format!("Create: {e:?}")));
            return;
        }
    };
    let Some(proj) = cfg.project(&create.project) else {
        hlog!("front: match {:#x}: unknown project '{}'", create.match_id, create.project);
        let _ = s.write_all(&fault_frame(&format!("unknown project '{}'", create.project)));
        return;
    };
    let _ = s.set_read_timeout(None);
    let bytes = match world_host_proto::frame_bytes(frame.ty, &frame.payload) {
        Ok(b) => b,
        Err(e) => {
            let _ = s.write_all(&fault_frame(&format!("Create: {e:?}")));
            return;
        }
    };
    // the room status counts this Create from now until it is handed over or refused
    let arriving = Arriving::new(&proj.name);
    // the memory limit (plan): hand over, load the map, evict idle maps, or wait for room (at most ROOM_WAIT); one
    // retry with a fresh zygote when a hand-over fails
    let t0 = Instant::now();
    let mut attempt = 0;
    let mut loads = 0;
    let mut waiting: Option<&'static str> = None;
    loop {
        reap_retired();
        let mut z = zygotes.lock().unwrap();
        drop_exited(&mut z);
        let v = views(cfg, &mut z);
        let wait = match plan(&proj.name, cfg.max_zygotes, &v, available_mb) {
            Plan::HandOver => None,
            // a zygote that died before it took the match is loaded once more, not again and again
            Plan::Load | Plan::LoadOver(_) if loads >= 2 => break,
            // the hand-over follows under the same lock: no other match can evict the new zygote before it counts as
            // busy (sent > taken)
            p @ (Plan::Load | Plan::LoadOver(_)) => match load_zygote(cfg, proj) {
                Ok(n) => {
                    loads += 1;
                    let over = match p {
                        Plan::LoadOver(mb) => format!(" one over max_zygotes (every loaded map runs matches, {mb} MB available)"),
                        _ => String::new(),
                    };
                    hlog!("front: load {}{over} for match {:#x} (pid {}, through the base); before: {}", proj.name, create.match_id, n.pid, counts(&v, cfg.max_zygotes));
                    z.insert(proj.name.clone(), n);
                    None
                }
                // no base process ready (starting, or restarted after it ended): wait for it like for room
                Err(LoadErr::NotReady(why)) => Some(why),
                Err(LoadErr::Failed(e)) => {
                    hlog!("front: zygote {}: {e}", proj.name);
                    let _ = s.write_all(&fault_frame(&format!("zygote: {e}")));
                    return;
                }
            },
            Plan::Evict(names) => {
                for name in names {
                    if let Some(old) = z.remove(&name) {
                        hlog!(
                            "front: evict {name} (pid {}, idle, last match {} s ago{}) to load {} for match {:#x}; before: {}",
                            old.pid,
                            old.last_used.elapsed().as_secs(),
                            if cfg.prestart.contains(&name) { ", prestart" } else { "" },
                            proj.name,
                            create.match_id,
                            counts(&v, cfg.max_zygotes)
                        );
                        // closing the control socket ends its loop (zygote_wait: Ok(None)); it runs no match, so it
                        // exits at once, and counts until it did
                        let Zygote { pid, pidfd, ctl, .. } = old;
                        drop(ctl);
                        RETIRED.lock().unwrap().push(Retired { name, pid, proc_: pidfd, since: Instant::now(), killed: false });
                    }
                }
                // the next turn waits until they exited, then loads
                continue;
            }
            Plan::Wait(why) => Some(why),
        };
        if let Some(why) = wait {
            if t0.elapsed() >= wait_limit(why) {
                hlog!("front: refuse match {:#x} ({}): no room after {} s, {why}; {}", create.match_id, proj.name, t0.elapsed().as_secs(), counts(&v, cfg.max_zygotes));
                drop(z);
                let _ = s.write_all(&fault_frame(refusal_text(why)));
                return;
            }
            if waiting != Some(why) {
                hlog!("front: match {:#x} ({}) waits for room: {why}; {}", create.match_id, proj.name, counts(&v, cfg.max_zygotes));
                waiting = Some(why);
            }
            drop(z);
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        let zy = z.get_mut(&proj.name).expect("loaded: planned on its view, or inserted");
        match crate::fdpass::send_fd(&zy.ctl, s.as_raw_fd(), &bytes) {
            Ok(()) => {
                zy.last_used = Instant::now();
                zy.sent = zy.sent.wrapping_add(1);
                let now = zy.last_used;
                zy.handed.retain(|t| now.saturating_duration_since(*t) < RECENT_FORK);
                zy.handed.push(now);
                // counted as handed over from here on (under the zygotes lock, as the status reads both)
                drop(arriving);
                hlog!("front: match {:#x} ({}) handed to zygote pid {}", create.match_id, proj.name, zy.pid);
                return;
            }
            Err(e) => {
                hlog!("front: hand-over to zygote {} failed (attempt {attempt}): {e}", proj.name);
                // killed through its pidfd; it counts (and its map waits) until it ended
                if let Some(old) = z.remove(&proj.name) {
                    pidfd_kill(&old.pidfd);
                    let Zygote { pid, pidfd, .. } = old;
                    RETIRED.lock().unwrap().push(Retired { name: proj.name.clone(), pid, proc_: pidfd, since: Instant::now(), killed: true });
                }
                attempt += 1;
                if attempt >= 2 {
                    break;
                }
            }
        }
    }
    let _ = s.write_all(&fault_frame("no zygote took the match"));
}

/// Why a zygote could not be had from the base process.
#[derive(Debug, PartialEq, Eq)]
enum LoadErr {
    /// no base process ready (it starts, or it ended and is started again): the match waits, as for room
    NotReady(&'static str),
    /// the base answered Failed, or a local error: the match is refused
    Failed(String),
}

const BASE_STARTING: &str = "the base process is starting";
const BASE_LOST: &str = "the base process ended during a request and starts again";
const BASE_BUSY: &str = "the base process refused the fork for now (an exe thread was not quiet in time)";

/// `why` is a wait for the base process (not for room).
fn is_base_wait(why: &str) -> bool {
    why == BASE_STARTING || why == BASE_LOST || why == BASE_BUSY
}

/// The Fault text of a match refused after its wait for `why`.
fn refusal_text(why: &str) -> &'static str {
    if is_base_wait(why) { "the host's base process is not ready" } else { "the host has no room for another map now" }
}

/// How long a match waits for `why`: BASE_START_WAIT for the base process, ROOM_WAIT for room.
fn wait_limit(why: &str) -> Duration {
    if is_base_wait(why) { BASE_START_WAIT } else { ROOM_WAIT }
}

/// The base process as the front runs it (started by `base_turn`).
struct BaseProc {
    child: Child,
    ctl: UnixStream,
    started: Instant,
    /// its Ready arrived
    ready: bool,
    /// it got a base pre-warm set, computed with the exclusion file at this size and time
    set_with: Option<Option<(u64, std::time::SystemTime)>>,
}

static BASE: Mutex<Option<BaseProc>> = Mutex::new(None);
/// A base with a pre-warm set ended before Ready: no more sets for this front's life (`base_set_file`).
static BASE_SET_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// When the last base process was started.
static BASE_STARTED: Mutex<Option<Instant>> = Mutex::new(None);

/// A new base process at most this often (one that ended is started again after this).
const BASE_RESTART_GAP: Duration = Duration::from_secs(5);
/// A base process that has not said Ready after this long is killed (and started again).
const BASE_READY_LIMIT: Duration = Duration::from_secs(180);
/// A match whose map needs loading waits this long at most for a base process that is not ready (starting, restarted,
/// or refusing a fork for now); the relay gives the host 45 s for Ready, and the level load follows.
const BASE_START_WAIT: Duration = Duration::from_secs(30);
/// The base's answer to a Load (a fork and a pidfd: the fork waits at most 0.25 s for its pollers and 0.25 s for its
/// timer threads). The route holds the zygotes lock meanwhile, so no longer.
const BASE_REPLY_WAIT: Duration = Duration::from_secs(3);
/// The base's Ready message, once its first byte is there.
const BASE_READY_READ: Duration = Duration::from_secs(1);
/// A ready base process older than this is replaced when no request is in flight (a new one starts; the old one ends
/// on its closed control socket; running zygotes are untouched). Every zygote inherits its base's monotonic clock, so
/// this bounds the clock a level load and its matches start at.
const BASE_MAX_AGE: Duration = Duration::from_secs(6 * 3600);

/// Base processes replaced by a younger one: reaped once they ended (killed if still alive RETIRE_KILL after).
static OLD_BASES: Mutex<Vec<(Child, Instant, bool)>> = Mutex::new(Vec::new());

/// A new base process is due: none was started yet, or the last one started at least BASE_RESTART_GAP ago.
fn base_start_due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|t| now.saturating_duration_since(t) >= BASE_RESTART_GAP)
}

/// The age at which a ready base is replaced: BASE_MAX_AGE, or `HOST_BASE_MAX_AGE_S=<seconds>` (diagnosis only, at
/// least BASE_MIN_AGE_DIAG, so that a run can see a replacement without waiting 6 h). Read once, logged once when set.
fn base_max_age() -> Duration {
    static AGE: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *AGE.get_or_init(|| match base_max_age_of(std::env::var("HOST_BASE_MAX_AGE_S").ok().as_deref()) {
        Some(d) => {
            hlog!("diagnosis: HOST_BASE_MAX_AGE_S: a ready base is replaced after {} s (instead of {} h)", d.as_secs(), BASE_MAX_AGE.as_secs() / 3600);
            d
        }
        None => BASE_MAX_AGE,
    })
}

/// The diagnosis value of HOST_BASE_MAX_AGE_S: whole seconds, raised to BASE_MIN_AGE_DIAG; None when unset or not a
/// number.
fn base_max_age_of(v: Option<&str>) -> Option<Duration> {
    v.and_then(|s| s.trim().parse::<u64>().ok()).map(|s| Duration::from_secs(s).max(BASE_MIN_AGE_DIAG))
}

/// The shortest base age HOST_BASE_MAX_AGE_S may set (a base start takes about 3 s; shorter would replace bases
/// back to back).
const BASE_MIN_AGE_DIAG: Duration = Duration::from_secs(60);

/// A base process started at `started` is to be replaced at `now` (`max`: base_max_age()).
fn base_too_old(started: Instant, now: Instant, max: Duration) -> bool {
    now.saturating_duration_since(started) >= max
}

/// Reap the replaced base processes that ended; kill one still alive RETIRE_KILL after it was replaced.
fn reap_old_bases() {
    OLD_BASES.lock().unwrap().retain_mut(|(c, since, killed)| match c.try_wait() {
        Ok(None) => {
            if !*killed && since.elapsed() >= RETIRE_KILL {
                hlog!("front: replaced base pid {} still alive {} s later: killed", c.id(), since.elapsed().as_secs());
                let _ = c.kill();
                *killed = true;
            }
            true
        }
        Ok(Some(st)) => {
            hlog!("front: replaced base pid {} exited: {st}", c.id());
            false
        }
        Err(_) => false,
    });
}

/// The base process's turn (front start, then every second): an ended base is logged and dropped; a ready one older
/// than BASE_MAX_AGE is replaced (no request is in flight: requests hold BASE); a starting one whose Ready arrived
/// becomes ready (one still not ready after BASE_READY_LIMIT is killed); with none, a new one is started when due.
fn base_turn(cfg: &DaemonCfg) {
    reap_old_bases();
    let mut g = BASE.lock().unwrap();
    if let Some(b) = g.as_mut()
        && let Ok(Some(st)) = b.child.try_wait()
    {
        hlog!("front: base pid {} exited: {st}; a new one starts within {} s", b.child.id(), BASE_RESTART_GAP.as_secs());
        if !b.ready && b.set_with.is_some() && !BASE_SET_FAILED.swap(true, std::sync::atomic::Ordering::Relaxed) {
            // it ended before Ready with a pre-warm set (an entry that cannot be queried before the level, say): the
            // next bases of this front start without one, so a bad entry never keeps the maps from loading
            hlog!("front: that base had a base pre-warm set and ended before Ready: the next bases start without one");
        }
        *g = None;
    }
    if let Some(b) = g.as_mut()
        && b.ready
        && base_too_old(b.started, Instant::now(), base_max_age())
    {
        hlog!(
            "front: base pid {} is {:.1} h old (BASE_MAX_AGE {} s), no request in flight: replaced (its zygotes go on; a new base starts)",
            b.child.id(),
            b.started.elapsed().as_secs_f64() / 3600.0,
            base_max_age().as_secs()
        );
        if let Some(old) = g.take() {
            // its closed control socket ends it ("the front closed the control socket")
            let BaseProc { child, ctl, .. } = old;
            drop(ctl);
            OLD_BASES.lock().unwrap().push((child, Instant::now(), false));
        }
    }
    if let Some(b) = g.as_mut()
        && b.ready
        && let Some(stamp) = b.set_with
        && file_stamp(&cfg.base_exclude_path()) != stamp
    {
        // a zygote's level load found an entry of this base's pre-warm set: a new base without it
        hlog!(
            "front: base pid {}: the base pre-warm set's exclusion file {} changed, no request in flight: replaced (its zygotes go on; a new base starts)",
            b.child.id(),
            cfg.base_exclude_path().display()
        );
        if let Some(old) = g.take() {
            let BaseProc { child, ctl, .. } = old;
            drop(ctl);
            OLD_BASES.lock().unwrap().push((child, Instant::now(), false));
        }
    }
    if let Some(b) = g.as_mut() {
        if b.ready {
            return;
        }
        let mut pfd = libc::pollfd { fd: b.ctl.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut pfd, 1, 0) } > 0 {
            let _ = b.ctl.set_read_timeout(Some(BASE_READY_READ));
            let r = crate::fdpass::recv_msg(&b.ctl).map(|m| m.map(|(_, d)| crate::base::FromBase::decode(&d)));
            let _ = b.ctl.set_read_timeout(None);
            match r {
                Ok(Some(Ok(crate::base::FromBase::Ready { pid }))) => {
                    b.ready = true;
                    hlog!("front: base ready after {:.1} s (pid {pid})", b.started.elapsed().as_secs_f64());
                }
                r => {
                    hlog!("front: base pid {}: no Ready ({r:?}); killed", b.child.id());
                    let _ = b.child.kill();
                }
            }
        } else if b.started.elapsed() >= BASE_READY_LIMIT {
            hlog!("front: base pid {} not ready after {} s: killed", b.child.id(), BASE_READY_LIMIT.as_secs());
            let _ = b.child.kill();
        }
        return;
    }
    let mut last = BASE_STARTED.lock().unwrap();
    if !base_start_due(*last, Instant::now()) {
        return;
    }
    *last = Some(Instant::now());
    match spawn_base(cfg) {
        Ok(b) => *g = Some(b),
        Err(e) => hlog!("front: cannot start the base process: {e}"),
    }
}

/// Start the base process: this binary with the zygotes' hook flags, its own output folder and its control socket as
/// descriptor 3 (`base_args`).
fn spawn_base(cfg: &DaemonCfg) -> io::Result<BaseProc> {
    let (ours, theirs) = UnixStream::pair()?;
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let out = cfg.out.join("base").join(format!("b{ms}"));
    std::fs::create_dir_all(&out)?;
    let exe = std::env::current_exe()?;
    let (set, set_with) = base_set_file(cfg, &out);
    let args = base_args(cfg, &out, &exe.to_string_lossy(), set.as_deref());
    let mut cmd = Command::new(&exe);
    cmd.args(&args[1..]);
    cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let fd = theirs.as_raw_fd();
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(move || {
            if fd == 3 {
                // dup2 onto itself keeps close-on-exec: clear it
                if libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
            } else if libc::dup2(fd, 3) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    drop(theirs);
    hlog!("front: base started, pid {}; its log {}", child.id(), out.join("host.log").display());
    Ok(BaseProc { child, ctl: ours, started: Instant::now(), ready: false, set_with })
}

/// The base pre-warm set of a new base (`DaemonCfg::base_prewarm_set`), written to `<out>/base-prewarm.tsv`: the file
/// (None: no set) and the exclusion file's stamp it was computed with. Logged either way.
fn base_set_file(cfg: &DaemonCfg, out: &std::path::Path) -> (Option<PathBuf>, Option<Option<(u64, std::time::SystemTime)>>) {
    if crate::baseset::off() {
        hlog!("front: base pre-warm set: none (diagnosis HOST_BASE_PREWARM=0)");
        return (None, None);
    }
    if BASE_SET_FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        hlog!("front: base pre-warm set: none (a base with a set ended before Ready)");
        return (None, None);
    }
    let stamp = file_stamp(&cfg.base_exclude_path());
    let (set, lists, excluded) = cfg.base_prewarm_set();
    let rule = match crate::baseset::lists_needed(lists, cfg.max_zygotes) {
        Some(k) => format!("in at least {k} of {lists} lists (max_zygotes {})", cfg.max_zygotes),
        None => format!("no entry can join: {lists} lists, max_zygotes {}", cfg.max_zygotes),
    };
    let x = if excluded > 0 { format!(", {excluded} left out by {}", cfg.base_exclude_path().display()) } else { String::new() };
    if set.is_empty() {
        hlog!("front: base pre-warm set: none ({rule}{x})");
        return (None, None);
    }
    let file = out.join("base-prewarm.tsv");
    let text: String = set.iter().map(|(c, n)| format!("{c:#x}\t{n}\n")).collect();
    if let Err(e) = std::fs::write(&file, text) {
        hlog!("front: base pre-warm set: {} not written: {e}; none", file.display());
        return (None, None);
    }
    hlog!("front: base pre-warm set: {} entries ({rule}{x}) -> {}", set.len(), file.display());
    (Some(file), Some(stamp))
}

/// A zygote for one project, from the base process: a control socket pair (ours stays, theirs goes with the Load), its
/// own folder (log, match folders) and the command line a zygote of its own would get (`zygote_args`); the base forks
/// and answers with the zygote's pid and a pidfd.
fn load_zygote(cfg: &DaemonCfg, p: &ProjectCfg) -> Result<Zygote, LoadErr> {
    if !BASE.lock().unwrap().as_ref().is_some_and(|b| b.ready) {
        return Err(LoadErr::NotReady(BASE_STARTING));
    }
    let failed = |e: String| LoadErr::Failed(e);
    let (ours, theirs) = UnixStream::pair().map_err(|e| failed(e.to_string()))?;
    // one folder per zygote (its log and match folders): a restarted front's new zygote never shares files with an old
    // one that still waits for its matches
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let out = cfg.out.join(&p.name).join(format!("z{ms}"));
    std::fs::create_dir_all(&out).map_err(|e| failed(format!("{}: {e}", out.display())))?;
    // the own list first takes in the best variant's; then the list the zygote loads (own, a variant's, or none), and
    // the own list every match grows (bp::record_prewarm)
    if let Some(line) = cfg.seed_prewarm(p) {
        hlog!("{line}");
    }
    // a project with neither its own nor a variant's list starts from the entries common to the lists of its kind
    let mut seeded = None;
    if common_seed_off() {
        hlog!("front: pre-warm list of {}: no common seed (diagnosis HOST_NO_COMMON_SEED=1)", p.name);
    } else if let Some((line, n)) = cfg.seed_common(p) {
        hlog!("{line}");
        seeded = n;
    }
    let mut pick = cfg.prewarm_pick(p);
    if let (Some(n), PrewarmSource::Own) = (seeded, &pick.source) {
        pick.source = PrewarmSource::Common(n);
    }
    let exe = std::env::current_exe().map_err(|e| failed(e.to_string()))?;
    let args = zygote_args(cfg, p, &pick, &out, &exe.to_string_lossy());
    let req = crate::base::ToBase::Load { project: p.name.clone(), args }.encode().map_err(failed)?;
    let mut g = BASE.lock().unwrap();
    let Some(b) = g.as_mut().filter(|b| b.ready) else {
        return Err(LoadErr::NotReady(BASE_STARTING));
    };
    let t0 = Instant::now();
    let r = (|| -> io::Result<_> {
        crate::fdpass::send_msg(&b.ctl, Some(theirs.as_raw_fd()), &req)?;
        b.ctl.set_read_timeout(Some(BASE_REPLY_WAIT))?;
        let m = crate::fdpass::recv_msg(&b.ctl);
        b.ctl.set_read_timeout(None)?;
        m
    })();
    drop(theirs);
    let reply = match r {
        Ok(Some((fd, data))) => (fd, crate::base::FromBase::decode(&data)),
        other => {
            let why = match other {
                Err(e) => e.to_string(),
                _ => "it closed its socket".to_string(),
            };
            // the base ended or does not answer: it is killed and started again (base_turn); the match waits for it
            hlog!("front: the base process (pid {}) did not answer the Load of {} ({why}): killed, a new one starts", b.child.id(), p.name);
            let _ = b.child.kill();
            b.ready = false;
            return Err(LoadErr::NotReady(BASE_LOST));
        }
    };
    match reply {
        (Some(pidfd), Ok(crate::base::FromBase::Loaded { project, pid })) if project == p.name => {
            hlog!(
                "front: zygote for {} ({}) started through the base, pid {pid}, in {} ms; pre-warm list: {}{}",
                p.name,
                p.level,
                t0.elapsed().as_millis(),
                pick.source,
                pick.list.as_ref().map_or(String::new(), |l| format!(", {} entries ({})", pick.entries, l.display()))
            );
            Ok(Zygote { pid, pidfd, ctl: ours, last_used: Instant::now(), sent: 0, status: (0, 0), handed: Vec::new(), loaded: Instant::now(), rx: Vec::new() })
        }
        (_, Ok(crate::base::FromBase::Failed { reason, .. })) => Err(LoadErr::Failed(format!("the base process: {reason}"))),
        // not forked now (an exe thread of the base was not quiet in time): the match waits and asks again
        (_, Ok(crate::base::FromBase::Busy { reason, .. })) => {
            hlog!("front: the base process refused the Load of {} for now ({reason}); the match waits and asks again", p.name);
            Err(LoadErr::NotReady(BASE_BUSY))
        }
        (fd, m) => {
            // out of step with the base: start a new one
            hlog!("front: unexpected answer of the base process to the Load of {}: {m:?} (descriptor {}): killed", p.name, fd.is_some());
            let _ = b.child.kill();
            b.ready = false;
            Err(LoadErr::NotReady(BASE_LOST))
        }
    }
}

/// The `prestart` maps, once the base process is ready (at most BASE_READY_LIMIT), each under the same limit as a match.
fn prestart(cfg: &DaemonCfg, zygotes: &Mutex<HashMap<String, Zygote>>) {
    if cfg.prestart.is_empty() {
        return;
    }
    let t0 = Instant::now();
    while !BASE.lock().unwrap().as_ref().is_some_and(|b| b.ready) {
        if t0.elapsed() >= BASE_READY_LIMIT {
            hlog!("front: prestart skipped: the base process is not ready after {} s", BASE_READY_LIMIT.as_secs());
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    for name in &cfg.prestart {
        let p = cfg.project(name).expect("checked by the config parser");
        let mut z = zygotes.lock().unwrap();
        let v = views(cfg, &mut z);
        if z.contains_key(name) {
            continue;
        }
        if cfg.max_zygotes != 0 && v.len() >= cfg.max_zygotes as usize {
            hlog!("front: refuse prestart {name}: {}", counts(&v, cfg.max_zygotes));
            continue;
        }
        match load_zygote(cfg, p) {
            Ok(n) => {
                hlog!("front: load {name} (prestart, pid {}, through the base); before: {}", n.pid, counts(&v, cfg.max_zygotes));
                z.insert(p.name.clone(), n);
            }
            Err(e) => hlog!("front: prestart {name}: {e:?}"),
        }
    }
}

/// The hook flags of every exe process of the daemon (the base process and the zygotes): steps 1-3 with the null render
/// world and the null cooks, the window import left at its first call, the network thread ended at its first call.
const HOOK_ARGS: &[&str] = &[
    "--steps",
    "3",
    "--render-world",
    "--null-render",
    "--null-cooks",
    "6,d,7b,render",
    "--hide-visuals",
    "--escape-at",
    "USER32.dll!LoadIconA",
    "--end-threads",
    ":network",
    "--quiet",
];

/// A zygote's whole command line (`argv0` first): what a zygote started on its own gets, and the list a zygote forked
/// from the base process reads instead of the base's (`cmdline`). `--zygote-fd 3`: the base replaces the 3 with the
/// descriptor it received.
pub fn zygote_args(cfg: &DaemonCfg, p: &ProjectCfg, pick: &PrewarmPick, out: &std::path::Path, argv0: &str) -> Vec<String> {
    let mut a: Vec<String> = vec![argv0.into(), "--exe".into(), cfg.exe.to_string_lossy().into(), "--exe-win".into(), cfg.exe_win.clone(), "--out".into(), out.to_string_lossy().into()];
    a.extend(HOOK_ARGS.iter().map(|s| s.to_string()));
    a.extend(["--level".into(), p.level.clone(), "--zygote-fd".into(), "3".into()]);
    a.extend(["--max-matches".into(), cfg.max_matches.to_string(), "--idle-secs".into(), cfg.idle_secs.to_string()]);
    a.extend(["--backstop-min".into(), cfg.backstop_min.to_string(), "--read-timeout".into(), cfg.read_timeout_secs.to_string()]);
    if cfg.tick_frames {
        a.push("--tick-frames".into());
    }
    if let Some(w) = &pick.list {
        a.extend(["--prewarm".into(), w.to_string_lossy().into(), "--prewarm-source".into(), pick.source.to_string()]);
    }
    if let Some(r) = &pick.record {
        a.extend(["--record-prewarm".into(), r.to_string_lossy().into()]);
    }
    a
}

/// The base process's command line (`argv0` first): the zygotes' hook flags, its own output folder, its control
/// socket as descriptor 3; no level and no zygote options. With a base pre-warm set (`set`): `--base-prewarm FILE` and
/// the exclusion file its zygotes grow, `--base-exclude FILE`.
pub fn base_args(cfg: &DaemonCfg, out: &std::path::Path, argv0: &str, set: Option<&std::path::Path>) -> Vec<String> {
    let mut a: Vec<String> = vec![argv0.into(), "--exe".into(), cfg.exe.to_string_lossy().into(), "--exe-win".into(), cfg.exe_win.clone(), "--out".into(), out.to_string_lossy().into()];
    a.extend(HOOK_ARGS.iter().map(|s| s.to_string()));
    a.extend(["--base-fd".into(), "3".into()]);
    if let Some(s) = set {
        a.extend(["--base-prewarm".into(), s.to_string_lossy().into(), "--base-exclude".into(), cfg.base_exclude_path().to_string_lossy().into()]);
    }
    a
}

// ---------------------------------------------------------------------------------------------------------------------
// base process

/// The base process (`world-host --base-fd N`, started by the front): steps 1-2 and step 3 up to the cut ran once
/// (`run::step3_base`, level-independent); here it tells the front Ready and then serves Load requests
/// ([`crate::base`]): per request it fork()s ([`crate::run::fork_world`], kind Zygote) and the child becomes the
/// zygote of that map: its own command line (the request's, `--zygote-fd` set to the received control socket), log and
/// output folder, then the rest of step 3 (the level, the pre-warm) and [`zygote_loop`], exactly as a zygote started on
/// its own. The base answers Loaded with a pidfd of the zygote (the front is not the zygote's parent), reaps its
/// zygotes and logs their exits. While it waits its world thread runs no exe code (idle parking on). It exits when the
/// front closes the control socket; its zygotes go on (their control sockets are with the front).
pub fn base_loop(o: &crate::run::Opts, fd: RawFd) -> i32 {
    // SAFETY: --base-fd names the control socket the front put at this descriptor for this process; nothing else
    // owns it
    let ctl = unsafe { UnixStream::from_raw_fd(fd) };
    let me = std::process::id();
    crate::run::mem_snapshot("base: at the cut");
    if std::env::var("HOST_MEM_BREAKDOWN").is_ok() {
        crate::run::mem_breakdown("base: at the cut");
    }
    crate::run::diag_memory("base: at the cut");
    hlog!("base: ready, pid {me}; cut after apply_startup_cfg at {:.3} s", crate::run::uptime_s());
    if let Err(e) = send_base(&ctl, None, &crate::base::FromBase::Ready { pid: me }) {
        hlog!("base: Ready to the front: {e}");
        return 1;
    }
    crate::k32::fork_gate_idle(true);
    crate::log::flush();
    let mut zygotes: HashMap<i32, String> = HashMap::new();
    loop {
        loop {
            let mut st = 0;
            let pid = unsafe { libc::waitpid(-1, &mut st, libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            let name = zygotes.remove(&pid).unwrap_or_else(|| "?".into());
            hlog!("base: zygote {name} pid {pid} exited, status {st:#x}");
        }
        let mut pfd = libc::pollfd { fd: ctl.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut pfd, 1, 1000) } <= 0 {
            continue;
        }
        let (zfd, data) = match crate::fdpass::recv_msg(&ctl) {
            Ok(Some(x)) => x,
            Ok(None) => {
                hlog!("base: the front closed the control socket, exiting ({} zygote(s) of this base still run)", zygotes.len());
                crate::k32::fork_gate_idle(false);
                return 0;
            }
            Err(e) => {
                // a cut message leaves the stream out of step: the front restarts the base
                hlog!("base: request from the front: {e}; exiting");
                crate::k32::fork_gate_idle(false);
                return 1;
            }
        };
        let (project, args) = match crate::base::ToBase::decode(&data) {
            Ok(crate::base::ToBase::Load { project, args }) => (project, args),
            Err(e) => {
                hlog!("base: request: {e}");
                let _ = send_base(&ctl, None, &crate::base::FromBase::Failed { project: String::new(), reason: e });
                continue;
            }
        };
        let r = match zfd {
            Some(z) => fork_zygote(&project, args, z),
            None => Err(Refusal::Failed("no control socket attached".to_string())),
        };
        let (pidfd, r) = match r {
            Ok(Forked::Child { args, from }) => {
                // the zygote: the base's control socket is not its own (its drop closes it here, once); the received
                // one is (its number is in --zygote-fd)
                drop(ctl);
                crate::run::zygote_child(o, args, &from);
            }
            Ok(Forked::Parent { pid, pidfd }) => {
                zygotes.insert(pid as i32, project.clone());
                (Some(pidfd), Ok(pid))
            }
            Err(e) => {
                match &e {
                    Refusal::Busy(why) => hlog!("base: zygote {project} refused for now: {why}"),
                    Refusal::Failed(why) => hlog!("base: zygote {project} failed: {why}"),
                }
                (None, Err(e))
            }
        };
        if let Err(e) = send_base(&ctl, pidfd.as_ref().map(|f| f.as_raw_fd()), &base_reply(&project, r.as_ref().copied())) {
            hlog!("base: reply to the front: {e}");
        }
        crate::log::flush();
    }
}

/// Why the base did not fork a zygote.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    /// not now (a fork gate or census TIMEOUT): the front asks again
    Busy(String),
    /// this request cannot be served
    Failed(String),
}

/// A fork of [`fork_zygote`]: in the base (the zygote's pid and a pidfd of it) or in the zygote (its command line).
enum Forked {
    Parent { pid: u32, pidfd: std::os::fd::OwnedFd },
    Child { args: Vec<String>, from: String },
}

/// The base's reply to a Load: Loaded with the zygote's pid, Busy (ask again) or Failed with the reason.
fn base_reply(project: &str, r: Result<u32, &Refusal>) -> crate::base::FromBase {
    let project = project.to_string();
    match r {
        Ok(pid) => crate::base::FromBase::Loaded { project, pid },
        Err(Refusal::Busy(reason)) => crate::base::FromBase::Busy { project, reason: reason.clone() },
        Err(Refusal::Failed(reason)) => crate::base::FromBase::Failed { project, reason: reason.clone() },
    }
}

fn send_base(s: &UnixStream, fd: Option<RawFd>, m: &crate::base::FromBase) -> io::Result<()> {
    let b = m.encode().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    crate::fdpass::send_msg(s, fd, &b)
}

/// One zygote: fork (a Zygote-kind fork, which is refused, Busy, after a fork gate or census TIMEOUT). In the child the
/// received control socket stays open for good (its number goes into --zygote-fd) and `Forked::Child` returns its
/// command line; the caller makes it the zygote. The parent closes its copy of the zygote's control socket and opens a
/// pidfd of the child for the front.
fn fork_zygote(project: &str, mut args: Vec<String>, zfd: std::os::fd::OwnedFd) -> Result<Forked, Refusal> {
    let t0 = Instant::now();
    hlog!("base: zygote for {project} requested");
    let out = crate::cmdline::value_in(&args, "--out").map(PathBuf::from).ok_or_else(|| Refusal::Failed("the zygote's command line has no --out".into()))?;
    if crate::cmdline::value_in(&args, "--level").is_none() {
        return Err(Refusal::Failed("the zygote's command line has no --level".into()));
    }
    std::fs::create_dir_all(&out).map_err(|e| Refusal::Failed(format!("{}: {e}", out.display())))?;
    crate::cmdline::set_value(&mut args, "--zygote-fd", &zfd.as_raw_fd().to_string());
    let from = format!("base pid {} (log {}/host.log)", std::process::id(), crate::run::out_dir());
    crate::log::flush();
    let pid = crate::run::fork_world(crate::run::ForkKind::Zygote, Some(&out.join("host.log"))).map_err(Refusal::Busy)?;
    if pid == 0 {
        // the zygote's control socket from now on: never closed by this process
        let _ = zfd.into_raw_fd();
        return Ok(Forked::Child { args, from });
    }
    drop(zfd);
    if pid < 0 {
        return Err(Refusal::Failed(format!("fork: {}", io::Error::last_os_error())));
    }
    // the base's own unreaped child: the pidfd names exactly this process
    let pfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
    if pfd < 0 {
        let e = io::Error::last_os_error();
        unsafe { libc::kill(pid, libc::SIGKILL) };
        return Err(Refusal::Failed(format!("pidfd_open: {e} (zygote pid {pid} killed)")));
    }
    hlog!("base: zygote {project} -> pid {pid} (pidfd sent) after {} ms", t0.elapsed().as_millis());
    // SAFETY: pidfd_open returned a new descriptor that nothing else owns
    Ok(Forked::Parent { pid: pid as u32, pidfd: unsafe { std::os::fd::OwnedFd::from_raw_fd(pfd) } })
}

// ---------------------------------------------------------------------------------------------------------------------
// zygote

/// Zygote options from the command line (set by the front).
#[derive(Clone, Debug)]
pub struct ZygoteOpts {
    pub ctl_fd: RawFd,
    pub max_matches: u32,
    pub idle_secs: u64,
    pub backstop_min: u32,
    pub read_timeout_secs: u64,
    pub tick_frames: bool,
}

impl ZygoteOpts {
    pub fn from_args() -> Option<ZygoteOpts> {
        let a = crate::cmdline::args();
        let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
        Some(ZygoteOpts {
            ctl_fd: get("--zygote-fd")? as RawFd,
            max_matches: get("--max-matches").unwrap_or(8) as u32,
            idle_secs: get("--idle-secs").unwrap_or(1800),
            backstop_min: get("--backstop-min").unwrap_or(65) as u32,
            read_timeout_secs: get("--read-timeout").unwrap_or(120),
            tick_frames: a.iter().any(|x| x == "--tick-frames"),
        })
    }
}

/// What the zygote pre-warmed (set before its loop, extended by every top-up; every match child logs it).
static PREWARM_NOTE: Mutex<String> = Mutex::new(String::new());

/// The entries the zygote queried at start (its `--prewarm` list as `run::prewarm` read it): its top-up skips them.
static PREWARM_LOADED: Mutex<Option<std::collections::HashSet<(u64, String)>>> = Mutex::new(None);

/// The zygote's pre-warm, after the level load and `prepare_process`: the `--prewarm` list the front picked
/// (`--prewarm-source`: own / sibling NAME), queried by [`crate::run::prewarm`]; logged, and kept for the children.
pub fn zygote_prewarm() -> Result<(), String> {
    let a = crate::cmdline::args();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    // the base pre-warm set: queried by the base already (its top-up skips it too)
    let base = crate::baseset::entries();
    let note = match get("--prewarm") {
        Some(p) => {
            let source = get("--prewarm-source").unwrap_or_else(|| "own".to_string());
            let entries = count_entries(std::path::Path::new(&p));
            let (kept, empty, skipped, text) = crate::run::prewarm_read_skipping(&p, &base)?;
            let mut loaded: std::collections::HashSet<(u64, String)> = prewarm_lines(&text).map(|(c, n)| (c, n.to_string())).collect();
            loaded.extend(base.iter().cloned());
            *PREWARM_LOADED.lock().unwrap() = Some(loaded);
            if base.is_empty() {
                format!("{source}, {entries} entries ({kept} kept, {empty} empty) from {p}")
            } else {
                format!("{source}, {entries} entries ({kept} kept, {empty} empty; {skipped} in the base, {} loaded here) from {p}", kept + empty)
            }
        }
        None => {
            if !base.is_empty() {
                *PREWARM_LOADED.lock().unwrap() = Some(base.iter().cloned().collect());
            }
            "none".to_string()
        }
    };
    let note = if base.is_empty() { note } else { format!("{note}; base pre-warm set {} entries", base.len()) };
    hlog!("zygote: pre-warm list: {note}");
    *PREWARM_NOTE.lock().unwrap() = note;
    Ok(())
}

/// The zygote's pre-warm, as [`zygote_prewarm`] logged it ("" outside a daemon zygote).
pub fn prewarm_note() -> String {
    PREWARM_NOTE.lock().map(|n| n.clone()).unwrap_or_default()
}

/// The zygote's loop, on the world thread after the level (and the pre-warm list) loaded. Returns when the front
/// closed the control socket or the zygote was idle for `idle_secs`; a child never returns from it.
pub fn zygote_loop(o: &ZygoteOpts) -> Result<(), String> {
    let ctl = unsafe { UnixStream::from_raw_fd(o.ctl_fd) };
    crate::run::mem_snapshot("zygote ready");
    if std::env::var("HOST_MEM_BREAKDOWN").is_ok() {
        crate::run::mem_breakdown("zygote ready");
    }
    crate::run::diag_memory("zygote ready");
    crate::registry::zygote_cache_line("zygote ready");
    hlog!("zygote: ready (max {} matches)", o.max_matches);
    let mut topup = TopUp::new();
    // from here on this thread runs no exe code (but in a top-up): idle engine worker pollers may block in Sleep (k32
    // fork gate)
    crate::k32::fork_gate_idle(true);
    crate::log::flush();
    let r = zygote_wait(o, ctl, &mut topup);
    crate::k32::fork_gate_idle(false);
    r
}

/// The (class, name) lines of a pre-warm list exactly as `run::prewarm` reads them: `CLASS<TAB>name` with a
/// hexadecimal class (a leading 0x is optional); every other line is skipped.
pub fn prewarm_lines(text: &str) -> impl Iterator<Item = (u64, &str)> {
    text.lines().filter_map(|l| {
        let (c, n) = l.split_once('\t')?;
        let c = u64::from_str_radix(c.trim_start_matches("0x"), 16).ok()?;
        Some((c, n))
    })
}

/// The entries of a list text a zygote's pre-warm queries ([`prewarm_lines`], in the file's order, repeats kept as
/// before) without those of `skip` (the base pre-warm set), and how many lines were skipped.
pub fn prewarm_plan(text: &str, skip: &std::collections::HashSet<(u64, String)>) -> (Vec<(u64, String)>, usize) {
    let (mut todo, mut skipped) = (Vec::new(), 0);
    for (c, n) in prewarm_lines(text) {
        if !skip.is_empty() && skip.contains(&(c, n.to_string())) {
            skipped += 1;
        } else {
            todo.push((c, n.to_string()));
        }
    }
    (todo, skipped)
}

/// The entries of the list text `text` that the zygote has not queried yet (`loaded`), in the file's order, each once.
pub fn topup_delta(loaded: &std::collections::HashSet<(u64, String)>, text: &str) -> Vec<(u64, String)> {
    let mut once = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (c, n) in prewarm_lines(text) {
        let e = (c, n.to_string());
        if !loaded.contains(&e) && once.insert(e.clone()) {
            out.push(e);
        }
    }
    out
}

/// The loop of `run::prewarm_entries`: `q(class, name)` for each entry in order (true = kept) until `elapsed()` has
/// reached `budget`, checked after each entry, so at least one is queried. Returns (kept, empty, queried).
pub fn prewarm_within(entries: &[(u64, String)], budget: Duration, elapsed: impl Fn() -> Duration, mut q: impl FnMut(u64, &str) -> bool) -> (u32, u32, usize) {
    let (mut kept, mut empty, mut n) = (0, 0, 0);
    for (c, name) in entries {
        if n > 0 && elapsed() >= budget {
            break;
        }
        if q(*c, name) {
            kept += 1;
        } else {
            empty += 1;
        }
        n += 1;
    }
    (kept, empty, n)
}

/// A top-up's share of an idle turn: it queries entries for at most this long; the rest waits for the next idle turn.
const TOPUP_BUDGET: Duration = Duration::from_millis(200);
/// The most a zygote grows by top-ups in its life (Private_Dirty, MB): then they stop (a zygote started from the
/// common seed grows by about 21 MB to hold its own first match's list; a bot match's kits are more).
const TOPUP_MAX_MB: u64 = 48;
/// MemAvailable (MB) a top-up needs: the rule for a match over max_matches (the running matches keep their room).
const TOPUP_FREE_MB: u64 = OVER_MATCHES_FREE_MB;

/// What a zygote's idle turn does about its pre-warm list ([`topup_turn`]).
#[derive(Debug, PartialEq, Eq)]
enum TopupTurn {
    /// the list did not change and nothing is left over
    Nothing,
    /// a child was forked within RECENT_FORK: its creation may still run and grow the list; ask again next turn
    Later,
    /// MemAvailable (MB, 0 = unreadable) under TOPUP_FREE_MB: ask again next turn
    Wait(u64),
    /// the zygote has grown by TOPUP_MAX_MB through top-ups: no more top-ups
    Stop,
    Go,
}

/// The decision of one idle turn: `changed` = the list file's size or time differs from the last read, `pending` =
/// the last top-up left entries for the next turn, `recent_fork` = a child was forked within RECENT_FORK, `grown_kb` =
/// what the top-ups so far added (Private_Dirty), `free_mb` = MemAvailable (read only when a top-up could run).
fn topup_turn(changed: bool, pending: bool, recent_fork: bool, grown_kb: u64, free_mb: impl FnOnce() -> Option<u64>) -> TopupTurn {
    if grown_kb >= TOPUP_MAX_MB * 1024 {
        return TopupTurn::Stop;
    }
    if !changed && !pending {
        return TopupTurn::Nothing;
    }
    if recent_fork {
        return TopupTurn::Later;
    }
    match free_mb() {
        Some(mb) if mb >= TOPUP_FREE_MB => TopupTurn::Go,
        mb => TopupTurn::Wait(mb.unwrap_or(0)),
    }
}

/// The size and time of a file (None when it cannot be read).
fn file_stamp(p: &std::path::Path) -> Option<(u64, std::time::SystemTime)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?))
}

/// This process's Private_Dirty (kB, /proc/self/smaps_rollup; 0 when unreadable).
fn private_dirty_kb() -> u64 {
    std::fs::read_to_string("/proc/self/smaps_rollup")
        .ok()
        .and_then(|r| r.lines().find(|l| l.starts_with("Private_Dirty:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}

/// The zygote's pre-warm top-up. A zygote reads its list once at start; the matches it forks then grow the project's
/// own list (their creation's misses and late loads). On an idle turn (no hand-over for a second) the zygote queries
/// the entries the file gained since, through the same exe query as at start (`run::prewarm_entries`), in the file's
/// order, with its own level loaded, so its later matches find them in the cache instead of cooking them each. Off
/// with the diagnosis switch HOST_PREWARM_TOPUP=0 and for a project without a recorded list (a `prewarm` file set by
/// hand records nothing).
struct TopUp {
    list: PathBuf,
    /// every entry this zygote has queried (at start and by top-ups)
    loaded: std::collections::HashSet<(u64, String)>,
    /// the list file's size and time when it was last read
    stamp: Option<(u64, std::time::SystemTime)>,
    /// the last top-up stopped at its time budget
    pending: bool,
    /// top-ups done; entries queried, kept, empty in all of them
    done: u32,
    total: (usize, u32, u32),
    grown_kb: u64,
    stopped: bool,
    waiting: bool,
    /// the start's pre-warm note (`prewarm_note`)
    note: String,
}

impl TopUp {
    fn new() -> Option<TopUp> {
        if std::env::var("HOST_PREWARM_TOPUP").is_ok_and(|v| v == "0") {
            hlog!("zygote: pre-warm top-up off (diagnosis HOST_PREWARM_TOPUP=0)");
            return None;
        }
        let Some(list) = crate::bp::record_prewarm_path() else {
            hlog!("zygote: pre-warm top-up: none (no recorded list for this project)");
            return None;
        };
        let loaded = PREWARM_LOADED.lock().unwrap().take().unwrap_or_default();
        hlog!("zygote: pre-warm top-up: follows {list} ({} entries queried at start)", loaded.len());
        Some(TopUp { list: PathBuf::from(list), loaded, stamp: None, pending: false, done: 0, total: (0, 0, 0), grown_kb: 0, stopped: false, waiting: false, note: prewarm_note() })
    }

    /// One idle turn of the zygote (its world thread runs no other exe code meanwhile, and no fork is in progress):
    /// at most one top-up, of at most TOPUP_BUDGET.
    fn turn(&mut self, recent_fork: bool) {
        if self.stopped {
            return;
        }
        let stamp = file_stamp(&self.list);
        let changed = stamp.is_some() && stamp != self.stamp;
        match topup_turn(changed, self.pending, recent_fork, self.grown_kb, available_mb) {
            TopupTurn::Nothing | TopupTurn::Later => return,
            TopupTurn::Stop => {
                self.stopped = true;
                hlog!("zygote: pre-warm top-up stopped: the top-ups added {} kB (TOPUP_MAX_MB {TOPUP_MAX_MB})", self.grown_kb);
                return;
            }
            TopupTurn::Wait(mb) => {
                if !self.waiting {
                    self.waiting = true;
                    hlog!("zygote: pre-warm top-up waits: MemAvailable {mb} MB, under {TOPUP_FREE_MB} MB");
                }
                return;
            }
            TopupTurn::Go => self.waiting = false,
        }
        let text = match std::fs::read_to_string(&self.list) {
            Ok(t) => t,
            Err(e) => {
                hlog!("zygote: pre-warm top-up: {}: {e}", self.list.display());
                self.stamp = stamp;
                return;
            }
        };
        self.stamp = stamp;
        let delta = topup_delta(&self.loaded, &text);
        if delta.is_empty() {
            self.pending = false;
            return;
        }
        // exe code on the world thread, as the start's pre-warm: the idle pollers poll again meanwhile
        crate::k32::fork_gate_idle(false);
        let pd0 = private_dirty_kb();
        let t0 = Instant::now();
        let (kept, empty, n) = crate::run::prewarm_entries(&delta, TOPUP_BUDGET);
        let ms = t0.elapsed().as_millis();
        // the start's pre-warm queued none (every zygote log); any queued now are retired as there
        let (retired, _) = crate::world::retire_render_commands();
        crate::k32::fork_gate_idle(true);
        let pd1 = private_dirty_kb();
        self.loaded.extend(delta[..n].iter().cloned());
        self.pending = n < delta.len();
        self.done += 1;
        self.total = (self.total.0 + n, self.total.1 + kept, self.total.2 + empty);
        self.grown_kb += pd1.saturating_sub(pd0);
        hlog!(
            "zygote: pre-warm top-up: {n} new entries ({kept} kept, {empty} empty) in {ms} ms; list now {} (top-up {}; the file has {}{}); \
             render commands retired {retired}; Private_Dirty {pd0} -> {pd1} kB",
            self.loaded.len(),
            self.done,
            prewarm_lines(&text).count(),
            if self.pending { format!("; {} left for the next idle turn", delta.len() - n) } else { String::new() }
        );
        crate::registry::zygote_cache_line(&format!("after top-up {}", self.done));
        *PREWARM_NOTE.lock().unwrap() = format!("{}; topped up by {} entries ({} kept, {} empty) in {} top-up(s)", self.note, self.total.0, self.total.1, self.total.2, self.done);
        crate::log::flush();
    }
}

/// The hand-over loop. After every change it tells the front its status (fdpass::send_status): hand-overs taken and
/// matches running; the front evicts a zygote only when it took every hand-over and runs no match. A turn without a
/// hand-over may top up the pre-warm list ([`TopUp`]).
fn zygote_wait(o: &ZygoteOpts, ctl: UnixStream, topup: &mut Option<TopUp>) -> Result<(), String> {
    let mut children: Vec<(u32, i32, Instant)> = Vec::new();
    let mut idle_since = Instant::now();
    let mut taken: u32 = 0;
    let mut told: Option<(u32, u32)> = None;
    loop {
        children.retain(|&(id, pid, at)| {
            let mut st = 0;
            if unsafe { libc::waitpid(pid, &mut st, libc::WNOHANG) } == pid {
                hlog!("zygote: match {id:#x} child {pid} ended, status {st:#x}, after {} s", at.elapsed().as_secs());
                false
            } else {
                true
            }
        });
        let now = (taken, children.len() as u32);
        if told != Some(now) {
            match crate::fdpass::send_status(&ctl, now.0, now.1) {
                Ok(true) => told = Some(now),
                // the front has not read for a long time (its buffer is full): sent again on the next turn
                Ok(false) => {}
                Err(e) => {
                    hlog!("zygote: status to the front: {e}");
                    told = Some(now);
                }
            }
        }
        if !children.is_empty() {
            idle_since = Instant::now();
        } else if idle_since.elapsed().as_secs() >= o.idle_secs {
            hlog!("zygote: idle for {} s, exiting", o.idle_secs);
            return Ok(());
        }
        let mut pfd = libc::pollfd { fd: ctl.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut pfd, 1, 1000) } <= 0 {
            // an idle turn: no hand-over waits and no fork runs
            if let Some(t) = topup.as_mut() {
                t.turn(children.iter().any(|c| c.2.elapsed() < RECENT_FORK));
            }
            continue;
        }
        let (conn, data) = match crate::fdpass::recv_fd(&ctl) {
            Ok(Some(x)) => x,
            Ok(None) => {
                // stay until the running matches end: while the zygote lives, its children share its pages (a child
                // whose zygote exited holds all of them privately: about 53 -> 416 MB private dirty memory)
                hlog!("zygote: the front closed the control socket; waiting for {} running match(es)", children.len());
                for (id, pid, at) in children.drain(..) {
                    let mut st = 0;
                    unsafe { libc::waitpid(pid, &mut st, 0) };
                    hlog!("zygote: match {id:#x} child {pid} ended, status {st:#x}, after {} s", at.elapsed().as_secs());
                }
                return Ok(());
            }
            Err(e) => {
                // a broken hand-over still counts as taken (the front counted it as sent)
                hlog!("zygote: hand-over: {e}");
                taken = taken.wrapping_add(1);
                continue;
            }
        };
        taken = taken.wrapping_add(1);
        let mut s = UnixStream::from(conn);
        let recent = children.iter().filter(|c| c.2.elapsed() < RECENT_FORK).count() as u32;
        let Some(over) = match_room(children.len() as u32, recent, o.max_matches, available_mb) else {
            let why = if children.len() as u32 >= o.max_matches.saturating_mul(2) { "twice max_matches" } else { "not enough memory" };
            hlog!("zygote: {} matches running (max_matches {}), {why}: another refused", children.len(), o.max_matches);
            let _ = s.write_all(&fault_frame("the host runs its maximum number of matches"));
            continue;
        };
        let frame = match parse_frame(&data) {
            Ok(f) => f,
            Err(e) => {
                let _ = s.write_all(&fault_frame(&e));
                continue;
            }
        };
        let match_id = match ToHost::decode(&frame) {
            Ok(ToHost::Create(c)) => c.match_id,
            _ => {
                let _ = s.write_all(&fault_frame("the first frame must be Create"));
                continue;
            }
        };
        if let Some(mb) = over {
            hlog!("zygote: match {match_id:#x} over max_matches {} ({} running, {mb} MB available)", o.max_matches, children.len());
        }
        crate::log::flush();
        // a match fork is never refused (Err only for a zygote)
        let pid = crate::run::fork_world(crate::run::ForkKind::Match, None).unwrap_or(-1);
        if pid == 0 {
            drop(ctl);
            let code = child_main(s, frame, o, over.is_some());
            crate::log::flush();
            unsafe { libc::_exit(code) };
        }
        if pid < 0 {
            hlog!("zygote: fork failed: {}", io::Error::last_os_error());
            let _ = s.write_all(&fault_frame("fork failed"));
            continue;
        }
        hlog!("zygote: match {match_id:#x} -> child {pid} ({} running)", children.len() + 1);
        children.push((match_id, pid, Instant::now()));
    }
}

fn parse_frame(b: &[u8]) -> Result<Frame, String> {
    let mut d = world_host_proto::FrameDecoder::new();
    d.push(b);
    let f = d.next_frame().map_err(|e| format!("frame: {e:?}"))?.ok_or("cut frame")?;
    if d.pending() != 0 {
        return Err("bytes after the Create frame".into());
    }
    Ok(f)
}

// ---------------------------------------------------------------------------------------------------------------------
// child

fn send(s: &mut UnixStream, m: &FromHost) -> io::Result<()> {
    let b = m.frame().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{e:?}")))?;
    s.write_all(&b)
}

/// One match, in a fork()ed child. Returns the exit code.
fn child_main(mut s: UnixStream, create: Frame, o: &ZygoteOpts, over: bool) -> i32 {
    let c = match ToHost::decode(&create) {
        Ok(ToHost::Create(c)) => c,
        _ => return 2,
    };
    // match id + pid: a relay may retry one match id, and test recordings share theirs
    let dir = PathBuf::from(crate::run::out_dir()).join(format!("m{:08x}-{}", c.match_id, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::log::open(dir.join("host.log").to_str().unwrap_or("host.log"));
    hlog!("child {}: match {:#x}, project '{}', kind {}, {} records", std::process::id(), c.match_id, c.project, c.kind, c.records.len());
    hlog!("child: the zygote's pre-warm list: {}", prewarm_note());
    // a match over max_matches is the first an OOM takes
    let _ = std::fs::write("/proc/self/oom_score_adj", if over { "800" } else { "500" });
    unsafe { libc::alarm(o.backstop_min.saturating_mul(60)) };
    let _ = s.set_read_timeout(Some(Duration::from_secs(o.read_timeout_secs)));
    if !(MIN_PROTO_VERSION..=PROTO_VERSION).contains(&c.version) {
        let _ = send(&mut s, &FromHost::Fault { reason: format!("protocol version {} (host speaks {MIN_PROTO_VERSION}..={PROTO_VERSION})", c.version) });
        return 2;
    }
    if c.world.is_empty() && c.version < 4 {
        let _ = send(&mut s, &FromHost::Fault { reason: format!("Create version {} without a world (the host builds one from version 4)", c.version) });
        return 2;
    }
    // what the match registers in the exe's name cache after its creation query (registry.rs: reads only); the summary
    // line and the list growth come at the end, on every way out of this function
    crate::registry::arm_late_trace();
    let _late_end = crate::registry::LateEnd;
    let t0 = Instant::now();
    // host bots (bots.rs): HOST_BOTS in the daemon's environment; before the creation (PvE user data +0x38)
    // protocol 5: `Create.drive_bots` asks for the host's bots (PvE: the mission's script bots); without it HOST_BOTS (tests)
    let kind_pve = c.kind == world_host_proto::KIND_PVE;
    crate::bots::set_plan(if c.drive_bots {
        if kind_pve { crate::bots::Plan::Pve } else { crate::bots::Plan::Pvp }
    } else {
        crate::bots::plan_from_env(kind_pve)
    });
    // protocol 4, an empty world: the host builds the start world itself from the options and the level (PvE: after one
    // step), and sends it in Ready
    let mut start = None;
    let made = if c.world.is_empty() {
        crate::world::MatchWorld::create_own(&c.options, c.kind == world_host_proto::KIND_PVE).map(|(m, item)| {
            let _ = std::fs::write(dir.join("start_item.bin"), &item);
            hlog!("child: own start world, {} bytes at {} (start_item.bin)", item.len(), m.present());
            start = Some(item);
            m
        })
    } else {
        crate::world::MatchWorld::create(&c.options, Some(&c.world))
    };
    let mut mw = match made {
        Ok(m) => m,
        Err(e) => {
            hlog!("child: create: {e}");
            let _ = send(&mut s, &FromHost::Fault { reason: format!("create: {e}").chars().take(256).collect() });
            return 1;
        }
    };
    crate::registry::late_check(crate::registry::Phase::BeforeReady, mw.present());
    // the kill log (protocol 3): a hardware breakpoint at the statistics handler's slot 3, armed once the match exists
    crate::kills::arm();
    let ready = Ready { match_id: c.match_id, world_time: mw.present(), collision_checksum: mw.collision_checksum(), create_ms: t0.elapsed().as_millis() as u32, start };
    hlog!(
        "child: ready match {:#x}, world time {}, collision checksum {:#x}, created in {} ms, start item {}",
        ready.match_id,
        ready.world_time,
        ready.collision_checksum,
        ready.create_ms,
        ready.start.as_ref().map_or("from the relay".to_string(), |b| format!("{} bytes", b.len()))
    );
    crate::run::mem_snapshot("child: match created");
    if std::env::var("HOST_MEM_BREAKDOWN").is_ok() {
        crate::run::mem_breakdown("child: match created");
    }
    crate::run::diag_memory("child: match created");
    if send(&mut s, &FromHost::Ready(ready)).is_err() {
        return 0;
    }
    let mut ticks = std::fs::File::create(dir.join("ticks.tsv")).ok().map(io::BufWriter::new);
    let cap = Captures::from_env(&dir);
    // the AI runs and its entries are collected per tick (bots.tsv)
    let mut bots = match crate::bots::HostBots::start(mw.core, &dir.to_string_lossy()) {
        Ok(b) => b,
        Err(e) => {
            hlog!("child: {e}; the match runs without host bots");
            None
        }
    };
    crate::registry::late_check(crate::registry::Phase::BotsStart, mw.present());
    // protocol 5: every reported bot entry must come back in a DF (the relay files it); the Bots frame goes out per step
    crate::bots::set_expect_echo(c.drive_bots && bots.is_some());
    // protocol 7: a custom room's "bots ignore players" = the exe's own harmless-bots byte, read by the AI every tick
    if c.ai_ignore_players {
        let b = crate::bots::set_ignore_players(true);
        hlog!("child: the AI ignores the human players (byte 0x20F43FE53 = {b}), host bots {}", if bots.is_some() { "on" } else { "off" });
    }
    let mut bots_diverged = false;
    let mut sess = Session::new(c.kind, SessionConfig { tick_frames: o.tick_frames, tick_ms: u32::from(c.tick_ms).max(1), used_frame: c.version >= 6, v8: c.version >= 8, ..Default::default() });
    loop {
        let f = match read_frame(&mut s) {
            Ok(Some(f)) => f,
            Ok(None) => {
                hlog!("child: EOF from the relay after {} steps", sess.steps);
                if let Some(b) = bots.as_mut() {
                    b.report(Some(&mw.results()));
                    b.flush();
                }
                return 0;
            }
            Err(e) => {
                hlog!("child: read: {e} (after {} steps)", sess.steps);
                return 1;
            }
        };
        let msg = match ToHost::decode(&f) {
            Ok(m) => m,
            Err(e) => {
                let _ = send(&mut s, &FromHost::Fault { reason: format!("bad frame: {e:?}") });
                return 1;
            }
        };
        let is_df = matches!(msg, ToHost::Df(_));
        let rep = sess.on_frame(&mut mw, msg);
        if let (true, Some(t)) = (is_df, sess.last_t) {
            crate::registry::late_check(crate::registry::Phase::InPlay, t);
        }
        if let (true, Some(cap), Some(t)) = (is_df, cap.as_ref(), sess.last_t) {
            cap.after_step(&mw, t);
        }
        if let (true, false, Some(t)) = (is_df, rep.done, sess.last_t) {
            let bt = bots.as_mut().map(|b| b.after_step(t));
            if c.drive_bots {
                // one Bots per stepped DF, empty included (the relay's hold of DF t + tick waits for it); a host whose
                // AI did not start sends empty frames, so the relay never waits in vain
                let frame = match bt.as_ref() {
                    Some(bt) => crate::bots::to_proto(bt),
                    None => world_host_proto::bots::Bots { t_for: t + u32::from(c.tick_ms).max(1), inputs: vec![], ai: vec![], offgrid: 0 },
                };
                if send(&mut s, &FromHost::Bots(frame)).is_err() {
                    hlog!("child: the relay is gone");
                    return 0;
                }
                let n = bots.as_ref().map_or(0, |b| b.divergence());
                if n > 0 && !bots_diverged {
                    bots_diverged = true;
                    hlog!("child: host bots diverged at {t} ({n} entries the clients do not have the same): Diverged");
                    let _ = send(&mut s, &FromHost::Diverged(world_host_proto::Diverged { t, retimed: 0, merged: n.min(u64::from(u32::MAX)) as u32 }));
                }
            }
        }
        if let (Some(w), Some(h)) = (ticks.as_mut(), rep.tick.as_ref()) {
            let hb: String = h.hash.iter().map(|b| format!("{b:02x}")).collect();
            let _ = writeln!(w, "{}\t{:06x}\t{}\t{:02x}{:02x}{:02x}", h.t, h.mask, if hb.is_empty() { "-" } else { &hb }, h.g[0], h.g[1], h.g[2]);
        }
        for m in &rep.out {
            match m {
                FromHost::Outcome(x) => {
                    hlog!("child: Outcome {x:?}");
                    let spent: Vec<String> = mw.results().players.iter().filter(|p| p.keys_spent != 0 || p.gold_spent != 0).map(|p| format!("slot {} keys {} gold {}", p.slot, p.keys_spent, p.gold_spent)).collect();
                    hlog!("child: paid respawns at the Outcome: {}", if spent.is_empty() { "none".to_string() } else { spent.join(", ") });
                }
                FromHost::Final(x) => {
                    hlog!("child: Final {x:?}");
                    let spent: Vec<String> = mw.results().players.iter().filter(|p| p.keys_spent != 0 || p.gold_spent != 0).map(|p| format!("slot {} keys {} gold {}", p.slot, p.keys_spent, p.gold_spent)).collect();
                    hlog!("child: paid respawns at the Final: {}", if spent.is_empty() { "none".to_string() } else { spent.join(", ") });
                }
                FromHost::Diverged(x) => hlog!("child: Diverged {x:?}"),
                FromHost::Kills(k) => {
                    for r in &k.records {
                        hlog!("child: kill {r:?}");
                    }
                }
                FromHost::Used(u) => hlog!("child: Used {u:?}"),
                FromHost::Fault { reason } => hlog!("child: Fault {reason}"),
                _ => {}
            }
            if send(&mut s, m).is_err() {
                hlog!("child: the relay is gone");
                return 0;
            }
        }
        if rep.done {
            if let Some(w) = ticks.as_mut() {
                let _ = w.flush();
            }
            if let Some(b) = bots.as_mut() {
                b.report(Some(&mw.results()));
                b.flush();
            }
            crate::run::mem_snapshot("child: end");
            crate::run::diag_memory("child: end");
            crate::registry::late_finish();
            hlog!("child: {}", mw.history_pool_report());
            hlog!(
                "child: done after {} steps; render commands retired {} ({} with the consumer's keep marks), render pool peak {} of {}",
                sess.steps,
                mw.render_retired,
                mw.render_marked,
                mw.render_pool_peak,
                crate::world::render_pool_cap()
            );
            return 0;
        }
    }
}

/// Diagnosis only (the replay's switches): HOST_CAPTURE_EVERY=N / HOST_CAPTURE_UNTIL=MS / HOST_CAPTURE_DELAY=MS
/// write the full item (a DE body, the client's world-save capture) of tick `t - delay` to `<match dir>/capture/` every
/// N ticks and for every tick up to UNTIL, as `replay` does, so a daemon match compares with the client captures.
struct Captures {
    dir: PathBuf,
    every: u32,
    until: u32,
    delay: u32,
}

impl Captures {
    fn from_env(match_dir: &std::path::Path) -> Option<Captures> {
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let c = Captures { dir: match_dir.join("capture"), every: env("HOST_CAPTURE_EVERY"), until: env("HOST_CAPTURE_UNTIL"), delay: env("HOST_CAPTURE_DELAY") };
        if c.every == 0 && c.until == 0 {
            return None;
        }
        let _ = std::fs::create_dir_all(&c.dir);
        Some(c)
    }

    fn after_step(&self, mw: &crate::world::MatchWorld, t: u32) {
        if t < self.delay {
            return;
        }
        let ct = t - self.delay;
        if ct <= self.until || (self.every != 0 && (ct / 50) % self.every == 0) {
            if let Some((it, b)) = mw.full_item(ct) {
                let _ = std::fs::write(self.dir.join(format!("full_{it}.bin")), &b);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// room status reader (test tool)

/// `world-host room-status --socket S [--every-ms N] [--count K]`: ask the front for its room status (`ListStatus`, as
/// the lobby does every 0.5 s) K times (default once), N ms apart (default 500), and print each answer
/// ([`room_status_text`]). Exit 1 when a read failed. For room checks without a lobby or game clients.
pub fn room_status_main(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let Some(sock) = get("--socket") else {
        eprintln!("usage: world-host room-status --socket S [--every-ms N] [--count K]");
        return 2;
    };
    let every = get("--every-ms").and_then(|v| v.parse().ok()).unwrap_or(500u64);
    let count = get("--count").and_then(|v| v.parse().ok()).unwrap_or(1u32);
    let t0 = Instant::now();
    let mut rc = 0;
    for i in 0..count {
        let t = t0.elapsed().as_secs_f64();
        match read_room_status(&sock) {
            Ok(st) => print!("{}", room_status_text(&st, t)),
            Err(e) => {
                println!("{t:.1} s: room status not read: {e}");
                rc = 1;
            }
        }
        let _ = io::stdout().flush();
        if i + 1 < count {
            std::thread::sleep(Duration::from_millis(every));
        }
    }
    rc
}

fn read_room_status(sock: &str) -> Result<world_host_proto::status::HostStatus, String> {
    let mut s = UnixStream::connect(sock).map_err(|e| format!("{sock}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(4))).map_err(|e| e.to_string())?;
    world_host_proto::write_frame(&mut s, world_host_proto::ty::LIST_STATUS, &[]).map_err(|e| e.to_string())?;
    let f = read_frame(&mut s).map_err(|e| e.to_string())?.ok_or("the front closed without an answer")?;
    if f.ty != world_host_proto::ty::STATUS {
        return Err(format!("the front answered with frame {:#04x}", f.ty));
    }
    world_host_proto::status::HostStatus::decode(&f).map_err(|e| e.to_string())
}

/// One room status as text: `T s: room status: <summary>`, then `  map P: running R, starting S, room ROOM, busy |
/// idle (eviction rank N), loaded L s` per loaded map, `  new map I: KIND, evicts E` per map that can load (or `  new
/// maps: none can load` / `... base process not ready`), and `  drain: P` when the front names one.
pub fn room_status_text(st: &world_host_proto::status::HostStatus, t: f64) -> String {
    use world_host_proto::status::NewMaps;
    let mut s = format!("{t:.1} s: room status: {}\n", st.summary());
    for m in &st.loaded {
        let idle = m.evict.map_or("busy".to_string(), |r| format!("idle (eviction rank {r})"));
        s.push_str(&format!("  map {}: running {}, starting {}, room {:?}, {idle}, loaded {} s\n", m.project, m.running, m.starting, m.room, m.loaded_secs));
    }
    match &st.new_maps {
        NewMaps::Room(v) => {
            for (i, n) in v.iter().enumerate() {
                s.push_str(&format!("  new map {}: {:?}, evicts {}\n", i + 1, n.kind, n.evicts));
            }
        }
        NewMaps::NoRoom => s.push_str("  new maps: none can load\n"),
        NewMaps::BaseNotReady => s.push_str("  new maps: none can load (base process not ready)\n"),
    }
    if let Some(d) = &st.drain {
        s.push_str(&format!("  drain: {d}\n"));
    }
    s
}

// ---------------------------------------------------------------------------------------------------------------------
// relay emulator (test tool)

/// `world-host relay-replay --socket S --rec FILE.srprec --project P [--kind pvp|pve] [--out DIR] [--want-de]`: play a
/// relay recording to the daemon as the relay would (Create from the recording's head, every DF record as `Df`, then
/// `Close` with the recording's close tick) and log every reply frame to `<out>/replies.txt`. Exit 0 when the host
/// answered `Ready` and `Final`.
pub fn relay_replay_main(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let (Some(sock), Some(rec), Some(project)) = (get("--socket"), get("--rec"), get("--project")) else {
        eprintln!("usage: world-host relay-replay --socket S --rec FILE --project P [--kind pvp|pve] [--out DIR] [--want-de]");
        return 2;
    };
    let kind = if get("--kind").as_deref() == Some("pve") { world_host_proto::KIND_PVE } else { world_host_proto::KIND_PVP };
    let out = PathBuf::from(get("--out").unwrap_or(".".into()));
    let _ = std::fs::create_dir_all(&out);
    let o = ReplayOpts {
        want_de: args.iter().any(|a| a == "--want-de"),
        drop_after: get("--drop-after").and_then(|v| v.parse().ok()),
        pace_us: get("--pace-us").and_then(|v| v.parse().ok()).unwrap_or(0),
        own_world: args.iter().any(|a| a == "--own-world"),
        match_id: get("--match-id").and_then(|v| crate::run::parse_u32(&v)),
    };
    match relay_replay(&sock, &rec, &project, kind, &out, &o) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(e) => {
            eprintln!("relay-replay: {e}");
            3
        }
    }
}

/// The recording: `SRPREC01, u32 match id, u32 world time, u8 tick, u8 n, n x {u8 bot, u8 team}, u32 len + options,
/// u32 len + DE item`, then records `u8 kind, u32 t, u32 len, DF bytes` (kind 0xFF = end, its t = the close tick).
pub struct Recording {
    pub match_id: u32,
    pub tick_ms: u8,
    pub records: Vec<world_host_proto::Record>,
    pub options: Vec<u8>,
    pub world: Vec<u8>,
    pub dfs: Vec<(u32, Vec<u8>)>,
    pub close_t: Option<u32>,
}

impl Recording {
    pub fn parse(d: &[u8]) -> Result<Recording, String> {
        let cut = || "recording cut".to_string();
        let u32_at = |p: usize| d.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(cut);
        if d.get(..8) != Some(&b"SRPREC01"[..]) {
            return Err("not SRPREC01".into());
        }
        let match_id = u32_at(8)?;
        let tick_ms = *d.get(16).ok_or_else(cut)?;
        let n = *d.get(17).ok_or_else(cut)? as usize;
        let rb = d.get(18..18 + 2 * n).ok_or_else(cut)?;
        let records = rb.chunks(2).map(|r| world_host_proto::Record { bot: r[0] != 0, team: r[1] }).collect();
        let mut p = 18 + 2 * n;
        let mut blob = || -> Result<Vec<u8>, String> {
            let l = u32_at(p)? as usize;
            let b = d.get(p + 4..p + 4 + l).ok_or_else(cut)?.to_vec();
            p += 4 + l;
            Ok(b)
        };
        let options = blob()?;
        let world = blob()?;
        let (mut dfs, mut close_t) = (Vec::new(), None);
        while p + 5 <= d.len() {
            let t = u32_at(p + 1)?;
            if d[p] == 0xFF {
                close_t = Some(t);
                break;
            }
            let l = u32_at(p + 5)? as usize;
            let Some(b) = d.get(p + 9..p + 9 + l) else { break };
            dfs.push((t, b.to_vec()));
            p += 9 + l;
        }
        // the relay's 4 closing DFs (relay.rs: `emit_df(t, fin = true)` on the 4 ticks before the recorder's close tick,
        // every recipient's leave bit) are never sent to the host on the live link: drop them, so a replay
        // ends at the last normal DF as the live host does
        let fin = fin_dfs(close_t, tick_ms, &dfs);
        dfs.truncate(dfs.len() - fin);
        let close_t = if fin > 0 { dfs.last().map(|x| x.0) } else { close_t };
        Ok(Recording { match_id, tick_ms, records, options, world, dfs, close_t })
    }
}

/// relay-replay switches: `--want-de` (a WantDe before Close), `--drop-after N` (close the connection after N DFs,
/// no Close), `--pace-us N` (sleep N microseconds after each DF: matches that overlap in time)
struct ReplayOpts {
    want_de: bool,
    drop_after: Option<usize>,
    pace_us: u64,
    /// `--own-world`: Create without the recording's world item (the host builds the start world itself)
    own_world: bool,
    /// `--match-id N`: Create.match_id and the options' match id replaced
    match_id: Option<u32>,
}

/// How many of the recording's last DFs are the relay's closing DFs: those on the 4 ticks before the close tick.
pub fn fin_dfs<T>(close_t: Option<u32>, tick_ms: u8, dfs: &[(u32, T)]) -> usize {
    let Some(c) = close_t else { return 0 };
    let from = c.saturating_sub(4 * tick_ms.max(1) as u32);
    dfs.iter().rev().take(4).take_while(|d| d.0 >= from && d.0 < c).count()
}

fn relay_replay(sock: &str, rec: &str, project: &str, kind: u8, out: &std::path::Path, o: &ReplayOpts) -> Result<bool, String> {
    let r = Recording::parse(&std::fs::read(rec).map_err(|e| format!("{rec}: {e}"))?)?;
    let mut s = UnixStream::connect(sock).map_err(|e| format!("{sock}: {e}"))?;
    let mut rd = s.try_clone().map_err(|e| e.to_string())?;
    let log_path = out.join("replies.txt");
    // the wire Tick frames (E0): "t mask hashes g0g1g2", as the host ticks.tsv columns 2..5
    let mut wire = io::BufWriter::new(std::fs::File::create(out.join("ticks_wire.tsv")).map_err(|e| e.to_string())?);
    // replies are read on their own thread (the host may answer while we still send)
    let reader = std::thread::spawn(move || -> (bool, bool, String) {
        let mut log = String::new();
        let (mut ready, mut fin) = (false, false);
        let mut ticks = 0u64;
        loop {
            match read_frame(&mut rd) {
                Ok(Some(f)) => match FromHost::decode(&f) {
                    Ok(FromHost::Tick(x)) => {
                        ticks += 1;
                        let m = x.mask[0] as u32 | (x.mask[1] as u32) << 8 | (x.mask[2] as u32) << 16;
                        let h: String = x.hashes.iter().map(|b| format!("{b:02x}")).collect();
                        let _ = writeln!(wire, "{}	{m:06x}	{}	{:02x}{:02x}{:02x}", x.t, if h.is_empty() { "-" } else { &h }, x.g[0], x.g[1], x.g[2]);
                    }
                    Ok(FromHost::De(d)) => log.push_str(&format!("De request {} time {} item {} bytes\n", d.request_id, d.time, d.item.len())),
                    Ok(FromHost::Ready(x)) => {
                        ready = true;
                        log.push_str(&format!(
                            "Ready match {:#x} world_time {} checksum {:#x} create_ms {} start {}\n",
                            x.match_id,
                            x.world_time,
                            x.collision_checksum,
                            x.create_ms,
                            x.start.as_ref().map_or("none".to_string(), |b| format!("{} bytes", b.len()))
                        ));
                    }
                    Ok(m) => {
                        ready |= matches!(m, FromHost::Ready(_));
                        fin |= matches!(m, FromHost::Final(_));
                        log.push_str(&format!("{m:?}\n"));
                    }
                    Err(e) => log.push_str(&format!("undecodable frame type {:#x}: {e:?}\n", f.ty)),
                },
                Ok(None) => {
                    log.push_str("EOF\n");
                    break;
                }
                Err(e) => {
                    log.push_str(&format!("read error {e}\n"));
                    break;
                }
            }
        }
        if ticks != 0 {
            log.push_str(&format!("{ticks} Tick frames\n"));
        }
        (ready, fin, log)
    });
    let mut options = r.options.clone();
    if let Some(id) = o.match_id {
        crate::run::set_options_match_id(&mut options, id)?;
    }
    let create = ToHost::Create(world_host_proto::Create {
        version: PROTO_VERSION,
        match_id: o.match_id.unwrap_or(r.match_id),
        kind,
        project: project.to_string(),
        map: 0,
        mode: 0,
        tick_ms: r.tick_ms,
        records: r.records.clone(),
        options,
        world: if o.own_world { Vec::new() } else { r.world.clone() },
        drive_bots: false,
        ai_ignore_players: false,
    });
    let t0 = Instant::now();
    let send = |s: &mut UnixStream, m: &ToHost| -> Result<(), String> { s.write_all(&m.frame().map_err(|e| format!("{e:?}"))?).map_err(|e| e.to_string()) };
    send(&mut s, &create)?;
    let mut sent = 0u32;
    if let Some(n) = o.drop_after {
        // the relay dies mid-match: send n DFs and close the connection without Close (the host child must exit)
        for (t, b) in r.dfs.iter().take(n) {
            send(&mut s, &ToHost::Df(world_host_proto::Df { t: *t, bytes: b.clone() }))?;
        }
        std::thread::sleep(Duration::from_millis(500));
        // shutdown, not drop: the reader thread's clone would keep the connection open
        let _ = s.shutdown(std::net::Shutdown::Both);
        let (ready, _, mut log) = reader.join().map_err(|_| "reader panicked")?;
        log.push_str(&format!("dropped the connection after {n} DFs\n"));
        std::fs::write(&log_path, &log).map_err(|e| e.to_string())?;
        print!("{log}");
        return Ok(ready);
    }
    // a host-built PvE world starts one step later (create_own first_step): the recording's DFs up to it are skipped
    let item_t = r.world.get(..4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let skip_upto = if o.own_world && kind == world_host_proto::KIND_PVE { item_t + u32::from(r.tick_ms) } else { 0 };
    for (t, b) in r.dfs.iter().filter(|d| d.0 > skip_upto) {
        if send(&mut s, &ToHost::Df(world_host_proto::Df { t: *t, bytes: b.clone() })).is_err() {
            break;
        }
        sent += 1;
        if o.pace_us != 0 {
            std::thread::sleep(Duration::from_micros(o.pace_us));
        }
    }
    if o.want_de {
        let _ = send(&mut s, &ToHost::WantDe { request_id: 1 });
    }
    let t_last = r.close_t.or(r.dfs.last().map(|x| x.0)).unwrap_or(0);
    let _ = send(&mut s, &ToHost::Close { t_last });
    let _ = s.shutdown(std::net::Shutdown::Write);
    let (ready, fin, mut log) = reader.join().map_err(|_| "reader panicked")?;
    log.push_str(&format!("sent {sent} of {} DFs, Close {t_last}; {} ms\n", r.dfs.len(), t0.elapsed().as_millis()));
    std::fs::write(&log_path, &log).map_err(|e| e.to_string())?;
    print!("{log}");
    Ok(ready && fin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_projects_and_refuses_unknown_keys() {
        let t = "socket = /tmp/w.sock\nexe = /g/survarium.exe\nexe_win = C:\\Games\\Survarium\\survarium.exe # comment\nmax_matches = 3\n\n[project pve_01_v1_maze2]\nlevel = pve_01/v1_maze2\nprewarm = /l/pve.tsv\n[project level_01_day_tdm]\nlevel = level_01/day_tdm\n";
        let c = DaemonCfg::parse(t).unwrap();
        assert_eq!(c.max_matches, 3);
        assert_eq!(c.projects.len(), 2);
        assert_eq!(c.project("pve_01_v1_maze2").unwrap().prewarm.as_deref(), Some(std::path::Path::new("/l/pve.tsv")));
        assert_eq!(c.project("level_01_day_tdm").unwrap().level, "level_01/day_tdm");
        assert!(DaemonCfg::parse("exe = a\nexe_win = b\nbogus = 1\n").is_err());
        assert!(DaemonCfg::parse("exe = a\nexe_win = b\n[project x]\n").is_err());
        assert!(DaemonCfg::parse("socket = /x\n").is_err());
        let p = DaemonCfg::parse("exe = a\nexe_win = b\nprestart = x, y\n[project x]\nlevel = l\n[project y]\nlevel = m\n").unwrap();
        assert_eq!(p.prestart, vec!["x".to_string(), "y".to_string()]);
        assert!(DaemonCfg::parse("exe = a\nexe_win = b\nprestart = z\n[project x]\nlevel = l\n").is_err());
        assert_eq!(p.max_zygotes, 0, "no cap by default");
        assert_eq!(DaemonCfg::parse("exe = a\nexe_win = b\nmax_zygotes = 8\n[project x]\nlevel = l\n").unwrap().max_zygotes, 8);
        // no project section and no game tables: refused
        assert!(DaemonCfg::parse("exe = a\nexe_win = b\n").unwrap_err().contains("data_dir"));
    }

    #[test]
    fn without_project_sections_the_projects_come_from_the_game_tables() {
        game_data::require_tables!();
        // the projects of the game data (107 shipped PvP rows + 4 Search and Destroy layouts + the gun race + 6 PvE mazes)
        let dir = game_data::testing::dir().unwrap();
        let d = DaemonCfg::parse(&format!("exe = a\nexe_win = b\nprestart = pve_01_v1_maze2\ndata_dir = {}\n", dir.display())).unwrap();
        assert_eq!(d.projects.len(), 118);
        assert_eq!(d.project("sad_abandoned_base").map(|p| p.level.as_str()), Some("sad_abandoned_base"));
        assert_eq!(d.project("arena_01").map(|p| p.level.as_str()), Some("arena_01/default"));
        assert_eq!(d.project("level_02_day_tdm").map(|p| p.level.as_str()), Some("level_02/day_tdm"));
        assert_eq!(d.project("pve_01_v1_maze2").map(|p| p.level.as_str()), Some("pve_01/v1_maze2"));
    }

    /// (project, live, idle, prestart, last match s after t0)
    fn zv(t0: Instant, v: &[(&str, bool, bool, bool, u64)]) -> Vec<ZyView> {
        v.iter().map(|&(p, live, idle, prestart, s)| ZyView { project: p.to_string(), live, idle, prestart, last_used: t0 + Duration::from_secs(s) }).collect()
    }

    /// MemAvailable for plan(): unknown (not Linux, or not read)
    fn no_mem() -> Option<u64> {
        None
    }

    #[test]
    fn a_loaded_map_takes_its_match_and_room_loads_a_new_one() {
        let t0 = Instant::now();
        let z = zv(t0, &[("pve", true, true, true, 0), ("a", true, false, false, 5)]);
        assert_eq!(plan("a", 2, &z, no_mem), Plan::HandOver, "loaded: hand over, even at the limit");
        assert_eq!(plan("b", 0, &z, no_mem), Plan::Load, "no limit");
        assert_eq!(plan("b", 3, &z, no_mem), Plan::Load, "2 + the new one fit in 3");
        assert_eq!(plan("b", 0, &[], no_mem), Plan::Load);
    }

    #[test]
    fn a_map_never_gets_a_second_zygote() {
        let t0 = Instant::now();
        // its evicted zygote still exits: wait, even with room and without a limit
        let z = zv(t0, &[("a", false, false, false, 0)]);
        assert_eq!(plan("a", 8, &z, no_mem), Plan::Wait("its evicted zygote has not exited yet"));
        assert_eq!(plan("a", 0, &z, no_mem), Plan::Wait("its evicted zygote has not exited yet"));
        assert_eq!(plan("b", 8, &z, no_mem), Plan::Load, "another map loads");
    }

    #[test]
    fn evicted_zygotes_count_until_they_exited() {
        let t0 = Instant::now();
        // limit 2: one busy live map + one exiting = full; the exiting one frees room soon: wait, evict nothing
        let z = zv(t0, &[("a", true, false, false, 0), ("b", false, false, false, 0)]);
        assert_eq!(plan("c", 2, &z, no_mem), Plan::Wait("an evicted map is still exiting"));
        // also with an idle live map: the exit already makes room, no second eviction
        let z = zv(t0, &[("a", true, true, false, 0), ("b", false, false, false, 0)]);
        assert_eq!(plan("c", 2, &z, no_mem), Plan::Wait("an evicted map is still exiting"));
        // limit 1, the one live map busy and one exiting (from before): the busy one is never evicted; nothing loads
        // over the limit while a map still exits
        let z = zv(t0, &[("a", true, false, false, 0), ("b", false, false, false, 0)]);
        assert_eq!(plan("c", 1, &z, no_mem), Plan::Wait("an evicted map is still exiting"));
        assert_eq!(plan("c", 1, &z, || panic!("read")), Plan::Wait("an evicted map is still exiting"));
    }

    #[test]
    fn idle_maps_are_evicted_oldest_first_prestart_last_busy_never() {
        let t0 = Instant::now();
        let z = zv(t0, &[("pve", true, true, true, 0), ("a", true, true, false, 5), ("b", true, true, false, 1), ("c", true, false, false, 0)]);
        assert_eq!(plan("d", 4, &z, no_mem), Plan::Evict(vec!["b".into()]), "the oldest idle non-prestart map makes room");
        assert_eq!(plan("d", 3, &z, no_mem), Plan::Evict(vec!["b".into(), "a".into()]), "a lowered limit evicts down to it");
        assert_eq!(plan("d", 2, &z, no_mem), Plan::Evict(vec!["b".into(), "a".into(), "pve".into()]), "a prestart map goes after every other idle one");
        assert_eq!(plan("d", 1, &z, no_mem), Plan::Evict(vec!["b".into(), "a".into(), "pve".into()]), "the busy map stays; the idle ones go down to the limit, memory or not");
        // a small server: max_zygotes 1, the prestart map idle -> it makes room for the match's map
        let z = zv(t0, &[("pve", true, true, true, 0)]);
        assert_eq!(plan("a", 1, &z, no_mem), Plan::Evict(vec!["pve".into()]));
        // ... and while it runs a match, the other map waits
        let z = zv(t0, &[("pve", true, false, true, 0)]);
        assert_eq!(plan("a", 1, &z, no_mem), Plan::Wait("the busy maps fill the limit"));
    }

    #[test]
    fn matches_over_max_matches_only_with_memory_and_at_most_twice_it() {
        assert_eq!(match_room(3, 3, 4, || panic!("read")), Some(None), "within the limit: memory not read");
        assert_eq!(match_room(4, 0, 4, || Some(4000)), Some(Some(4000)), "at the limit, memory available");
        assert_eq!(match_room(7, 0, 4, || Some(OVER_MATCHES_FREE_MB)), Some(Some(OVER_MATCHES_FREE_MB)));
        assert_eq!(match_room(8, 0, 4, || panic!("read")), None, "twice the limit: never more");
        assert_eq!(match_room(4, 0, 4, || Some(OVER_MATCHES_FREE_MB - 1)), None, "too little memory");
        assert_eq!(match_room(4, 0, 4, no_mem), None, "memory unknown");
        assert_eq!(match_room(0, 0, 0, || panic!("read")), None, "max_matches 0 takes none");
        // fresh children do not show their memory yet: a match's worth more per fork of the last seconds
        assert_eq!(match_room(4, 2, 4, || Some(OVER_MATCHES_FREE_MB + 2 * MATCH_MB - 1)), None);
        assert_eq!(match_room(4, 2, 4, || Some(OVER_MATCHES_FREE_MB + 2 * MATCH_MB)), Some(Some(OVER_MATCHES_FREE_MB + 2 * MATCH_MB)));
    }

    #[test]
    fn every_map_busy_loads_one_over_the_limit_only_with_memory() {
        let t0 = Instant::now();
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1)]);
        assert_eq!(plan("c", 2, &z, || Some(4000)), Plan::LoadOver(4000), "busy maps at the limit, memory available");
        assert_eq!(plan("c", 2, &z, || Some(OVER_CAP_FREE_MB)), Plan::LoadOver(OVER_CAP_FREE_MB));
        assert_eq!(plan("c", 2, &z, || Some(OVER_CAP_FREE_MB - 1)), Plan::Wait("the busy maps fill the limit"), "too little memory");
        assert_eq!(plan("c", 2, &z, no_mem), Plan::Wait("the busy maps fill the limit"), "memory unknown");
        // already one over: never a second one
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1), ("c", true, false, false, 2)]);
        assert_eq!(plan("d", 2, &z, || Some(64000)), Plan::Wait("the busy maps fill the limit"));
        // one over and idle, the others busy: it goes, then the new map is the one over (memory permitting)
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1), ("c", true, true, false, 2)]);
        assert_eq!(plan("d", 2, &z, || panic!("read")), Plan::Evict(vec!["c".into()]), "back to the limit without reading memory");
        // ... it exits: wait; then, at the limit, one over with memory
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1), ("c", false, false, false, 2)]);
        assert_eq!(plan("d", 2, &z, || panic!("read")), Plan::Wait("an evicted map is still exiting"));
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1)]);
        assert_eq!(plan("d", 2, &z, || Some(64000)), Plan::LoadOver(64000));
        // limit 1, three idle and one busy: evicting the idle ones down to the limit, then one over
        let z = zv(t0, &[("a", true, true, false, 0), ("b", true, true, false, 1), ("c", true, true, false, 2), ("x", true, false, false, 3)]);
        assert_eq!(plan("d", 1, &z, no_mem), Plan::Evict(vec!["a".into(), "b".into(), "c".into()]));
        // one over, then a map idles: the next new map evicts back down to the limit
        let z = zv(t0, &[("a", true, true, false, 0), ("b", true, true, false, 1), ("c", true, false, false, 2)]);
        assert_eq!(plan("d", 2, &z, || Some(64000)), Plan::Evict(vec!["a".into(), "b".into()]));
        // an idle map is evicted before any load over the limit
        let z = zv(t0, &[("a", true, true, false, 0), ("b", true, false, false, 1)]);
        assert_eq!(plan("c", 2, &z, || Some(64000)), Plan::Evict(vec!["a".into()]));
        // an exiting map: wait for it, no load over the limit
        let z = zv(t0, &[("a", true, false, false, 0), ("b", false, false, false, 1)]);
        assert_eq!(plan("c", 2, &z, || Some(64000)), Plan::Wait("an evicted map is still exiting"));
        // the memory is read only when every loaded map is busy at the limit
        assert_eq!(plan("a", 2, &z, || panic!("read")), Plan::HandOver);
        assert_eq!(plan("c", 0, &z, || panic!("read")), Plan::Load);
    }

    fn zl(project: &str, running: u32, starting: u32, recent: u32) -> ZyLoad {
        ZyLoad { project: project.into(), running, starting, recent, loaded_secs: 60 }
    }

    #[test]
    fn the_room_status_takes_each_loaded_maps_rule() {
        use world_host_proto::status::{MatchRoom, NewMaps};
        let t0 = Instant::now();
        let z = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1), ("c", true, false, false, 2), ("d", true, true, false, 3)]);
        let loads = [zl("a", 3, 0, 0), zl("b", 3, 1, 1), zl("c", 7, 1, 1), zl("d", 0, 0, 0)];
        let room = |mb: Option<u64>| room_status(4, 4, true, &z, &loads, &[], mb).loaded.iter().map(|m| m.room).collect::<Vec<_>>();
        // within max_matches; at it (running + starting) over it with memory for one more and the fresh fork; twice it never
        assert_eq!(room(Some(OVER_MATCHES_FREE_MB + MATCH_MB)), [MatchRoom::Within, MatchRoom::Over, MatchRoom::Full, MatchRoom::Within]);
        assert_eq!(room(Some(OVER_MATCHES_FREE_MB + MATCH_MB - 1)), [MatchRoom::Within, MatchRoom::Full, MatchRoom::Full, MatchRoom::Within], "the fresh fork's memory");
        assert_eq!(room(None), [MatchRoom::Within, MatchRoom::Full, MatchRoom::Full, MatchRoom::Within], "memory unknown: only within the limit");
        let st = room_status(4, 4, true, &z, &loads, &[], None);
        assert_eq!((st.loaded[1].running, st.loaded[1].starting, st.max_zygotes, st.max_matches), (3, 1, Some(4), 4));
        // only the idle map has an eviction rank; every map its age
        assert_eq!(st.loaded.iter().map(|m| m.evict).collect::<Vec<_>>(), [None, None, None, Some(0)]);
        assert_eq!(st.loaded[0].loaded_secs, 60);
        // max_matches 0 takes none
        assert!(room_status(4, 0, true, &z, &[zl("a", 0, 0, 0)], &[], Some(64000)).loaded.iter().all(|m| m.room == MatchRoom::Full));
        // no base process: no new map, whatever the limit
        assert_eq!(room_status(0, 4, false, &z, &loads, &[], Some(64000)).new_maps, NewMaps::BaseNotReady);
    }

    #[test]
    fn the_room_status_lists_the_maps_that_can_load_now() {
        use world_host_proto::status::{LoadKind, NewMap, NewMaps, MAX_NEW_MAPS};
        let t0 = Instant::now();
        let w = |evicts: u16| NewMap { kind: LoadKind::Within, evicts };
        let over = |evicts: u16| NewMap { kind: LoadKind::Over, evicts };
        let new = |cap: u32, z: &[ZyView], arriving: &[(String, u32)], mb: Option<u64>| room_status(cap, 4, true, z, &[], arriving, mb).new_maps;
        let busy2 = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1)]);
        assert_eq!(new(0, &busy2, &[], None), NewMaps::Room(vec![w(0); MAX_NEW_MAPS]), "no limit");
        assert_eq!(new(4, &busy2, &[], None), NewMaps::Room(vec![w(0), w(0)]), "two free slots, no memory for one over");
        // two free slots, then one over with the memory left after both loads
        assert_eq!(new(4, &busy2, &[], Some(OVER_CAP_FREE_MB + 2 * ZYGOTE_MB)), NewMaps::Room(vec![w(0), w(0), over(0)]));
        assert_eq!(new(4, &busy2, &[], Some(OVER_CAP_FREE_MB + 2 * ZYGOTE_MB - 1)), NewMaps::Room(vec![w(0), w(0)]));
        // at the limit, every map busy: one over with memory, else none
        assert_eq!(new(2, &busy2, &[], Some(OVER_CAP_FREE_MB)), NewMaps::Room(vec![over(0)]));
        assert_eq!(new(2, &busy2, &[], Some(OVER_CAP_FREE_MB - 1)), NewMaps::NoRoom);
        assert_eq!(new(2, &busy2, &[], None), NewMaps::NoRoom);
        // the idle maps can be evicted (least recently used first, prestart last): one new map each, its eviction with it
        let mixed = zv(t0, &[("a", true, true, false, 0), ("b", true, false, false, 1), ("c", true, true, true, 2)]);
        assert_eq!(new(3, &mixed, &[], None), NewMaps::Room(vec![w(1), w(1)]));
        let loads = [zl("a", 0, 0, 0), zl("b", 2, 0, 0), zl("c", 0, 0, 0)];
        let st = room_status(3, 4, true, &mixed, &loads, &[], None);
        assert_eq!(st.loaded.iter().map(|m| m.evict).collect::<Vec<_>>(), [Some(0), None, Some(1)]);
        // a Create on its way to the idle map a keeps it busy: no rank, only c can be evicted
        let on_a = vec![("a".to_string(), 1)];
        let st = room_status(3, 4, true, &mixed, &loads, &on_a, None);
        assert_eq!(st.loaded.iter().map(|m| m.evict).collect::<Vec<_>>(), [None, None, Some(0)]);
        assert_eq!(st.new_maps, NewMaps::Room(vec![w(1)]));
        // one over the limit that went idle: evicting it only gets back to the limit; a new map then needs the memory
        let back = zv(t0, &[("a", true, false, false, 0), ("b", true, false, false, 1), ("c", true, true, false, 2)]);
        assert_eq!(new(2, &back, &[], None), NewMaps::NoRoom);
        assert_eq!(new(2, &back, &[], Some(OVER_CAP_FREE_MB)), NewMaps::Room(vec![over(1)]));
        // a Create arriving for a map that has no zygote takes a slot already
        let arriving = vec![("x".to_string(), 1)];
        assert_eq!(new(3, &busy2, &arriving, None), NewMaps::NoRoom);
        assert_eq!(new(4, &busy2, &arriving, None), NewMaps::Room(vec![w(0)]));
        // ... one arriving for a busy loaded map does not
        assert_eq!(new(3, &busy2, &[("a".to_string(), 2)], None), NewMaps::Room(vec![w(0)]));
        // an evicted map still exiting: wait (no new map now)
        let exiting = zv(t0, &[("a", true, false, false, 0), ("b", false, false, false, 1)]);
        assert_eq!(new(2, &exiting, &[], Some(64000)), NewMaps::NoRoom);
    }

    #[test]
    fn one_busy_map_drains_at_a_time_and_a_stuck_one_passes_its_turn() {
        let t0 = Instant::now();
        let s = |v: &str| Some(v.to_string());
        let mut d = DrainTurn::default();
        assert_eq!(d.turn(false, &["a", "b"], t0), None, "a map can load: no drain");
        assert_eq!(d.turn(true, &["a", "b"], t0), s("a"), "the longest loaded");
        assert_eq!(d.turn(true, &["a", "b"], t0 + Duration::from_secs(60)), s("a"), "it stays while busy");
        // a idled (no longer a candidate): drained; the next longest loaded
        let t1 = t0 + Duration::from_secs(120);
        assert_eq!(d.turn(true, &["b", "c"], t1), s("b"));
        // b stays busy a whole turn: c gets its turn; c too: every one passed over, start again with the longest loaded
        assert_eq!(d.turn(true, &["b", "c"], t1 + DRAIN_TURN - Duration::from_secs(1)), s("b"));
        assert_eq!(d.turn(true, &["b", "c"], t1 + DRAIN_TURN), s("c"));
        assert_eq!(d.turn(true, &["b", "c"], t1 + DRAIN_TURN * 2), s("b"));
        assert!(d.stuck.is_empty());
        // room: no drain, the turn starts over; no busy candidate (only prestart maps busy): none
        assert_eq!(d.turn(false, &["b", "c"], t1), None);
        assert!(d.current.is_none() && d.stuck.is_empty());
        assert_eq!(d.turn(true, &[], t1), None);
    }

    #[test]
    fn arriving_creates_are_counted_while_they_live() {
        let on_way = |p: &str| ARRIVING.lock().unwrap().iter().find(|(a, _)| a == p).map_or(0, |a| a.1);
        let a = Arriving::new("room_test_a");
        let b = Arriving::new("room_test_a");
        assert_eq!(on_way("room_test_a"), 2);
        drop(a);
        assert_eq!(on_way("room_test_a"), 1);
        drop(b);
        assert_eq!(on_way("room_test_a"), 0);
        assert!(!ARRIVING.lock().unwrap().iter().any(|(p, _)| p == "room_test_a"), "an empty entry goes");
    }

    #[test]
    fn the_prewarm_list_keeps_unique_name_keyed_classes() {
        let long = "x".repeat(119);
        let q = format!("class\tname\ttid\tcaller\n0x68\tpvp_match_resource\t1\t0x2\n0x1f\ta.player\t1\t0x2\n0x31\tanim\t1\t0x2\n0x1f\ta.player\t2\t0x3\n0x43\titem\t1\t0x2\n0x13\t{long}\t1\t0x2\n0x13\t\t1\t0x2\n");
        assert_eq!(prewarm_list(&q, &PREWARM_CLASSES), "0x1f\ta.player\n0x31\tanim\n");
    }

    #[test]
    fn a_project_without_a_list_takes_the_shipped_one() {
        let dir = std::env::temp_dir().join(format!("wh-prewarm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("p1.tsv"), "0x1f\ta\n").unwrap();
        let t = format!("exe = a\nexe_win = b\nprewarm_dir = {}\n[project p1]\nlevel = l1\n[project p2]\nlevel = l2\n[project p3]\nlevel = l3\nprewarm = /own.tsv\n", dir.display());
        let c = DaemonCfg::parse(&t).unwrap();
        assert_eq!(c.prewarm_of(c.project("p1").unwrap()), Some(dir.join("p1.tsv")));
        assert_eq!(c.prewarm_of(c.project("p2").unwrap()), None, "p2 records its list at its first match");
        assert_eq!(c.prewarm_of(c.project("p3").unwrap()), Some(PathBuf::from("/own.tsv")));
        // the top-up follows the recorded list: a project with a configured file records nothing, so it has none
        assert_eq!(c.prewarm_pick(c.project("p2").unwrap()).record, Some(dir.join("p2.tsv")));
        assert_eq!(c.prewarm_pick(c.project("p3").unwrap()).record, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_top_up_reads_list_lines_as_the_start_does() {
        let t = "0x1f\ta.player\nbad line\n0xzz\tq\n0x3\t\n31\tanim\r\n0x1F\tB\n";
        let v: Vec<(u64, &str)> = prewarm_lines(t).collect();
        assert_eq!(v, vec![(0x1f, "a.player"), (0x3, ""), (0x31, "anim"), (0x1f, "B")]);
    }

    #[test]
    fn a_zygote_skips_the_base_set_and_queries_the_rest_as_before() {
        let t = "0x1f\ta\nbad line\n0x31\tb\n0x1f\ta\n0x3\tc\n";
        // no set: every line in the file's order, repeats kept (the pre-warm as it always ran)
        let none = std::collections::HashSet::new();
        assert_eq!(prewarm_plan(t, &none), (vec![(0x1f, "a".into()), (0x31, "b".into()), (0x1f, "a".into()), (0x3, "c".into())], 0));
        let base: std::collections::HashSet<(u64, String)> = [(0x1f, "a".to_string()), (0x2c, "x".to_string())].into_iter().collect();
        assert_eq!(prewarm_plan(t, &base), (vec![(0x31, "b".into()), (0x3, "c".into())], 2));
    }

    #[test]
    fn the_base_set_comes_from_every_own_list_once_without_the_exclusions() {
        let dir = std::env::temp_dir().join(format!("wh-baseset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.tsv"), "0x1f\tcommon\n0x31\tab\n0x3\tonly_a\n").unwrap();
        std::fs::write(dir.join("b.tsv"), "0x31\tab\n0x1f\tcommon\n").unwrap();
        std::fs::write(dir.join("c.tsv"), "0x1f\tcommon\n0x2c\tc\n").unwrap();
        let cfg = |cap: u32| {
            let t = format!(
                "exe = a\nexe_win = b\nmax_zygotes = {cap}\nprewarm_dir = {}\n[project a]\nlevel = l/a\n[project b]\nlevel = l/b\n[project c]\nlevel = l/c\n\
                 [project d]\nlevel = l/d\n[project e]\nlevel = l/e\nprewarm = {}\n",
                dir.display(),
                dir.join("c.tsv").display()
            );
            DaemonCfg::parse(&t).unwrap()
        };
        // c's file is e's configured list too: counted once; d has no list
        assert_eq!(cfg(0).base_prewarm_set(), (vec![(0x1f, "common".into()), (0x31, "ab".into())], 3, 0));
        // two maps at most: in all three lists only
        assert_eq!(cfg(2).base_prewarm_set(), (vec![(0x1f, "common".into())], 3, 0));
        std::fs::write(dir.join("base-exclude.tsv"), "0x31\tab\n").unwrap();
        assert_eq!(cfg(0).base_prewarm_set(), (vec![(0x1f, "common".into())], 3, 1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_top_up_takes_the_entries_not_loaded_in_file_order_each_once() {
        let loaded: std::collections::HashSet<(u64, String)> = [(0x1f, "a".to_string()), (0x31, "b".to_string())].into_iter().collect();
        let t = "0x1f\ta\n0xe\tc\n0x31\tb\n0x1F\ta\n0x2c\td\n0xe\tc\n0x31\ta\n";
        assert_eq!(topup_delta(&loaded, t), vec![(0xe, "c".to_string()), (0x2c, "d".to_string()), (0x31, "a".to_string())]);
        // nothing new: nothing to query
        assert!(topup_delta(&loaded, "0x1f\ta\n0x31\tb\n").is_empty());
        // a zygote that loaded nothing takes the whole file
        assert_eq!(topup_delta(&Default::default(), "0x1f\ta\n").len(), 1);
    }

    #[test]
    fn a_top_up_stops_at_its_time_budget_and_leaves_the_rest() {
        let e: Vec<(u64, String)> = (0..5).map(|i| (0x1f, format!("n{i}"))).collect();
        // every query takes 100 ms of the clock
        let clock = std::cell::Cell::new(Duration::ZERO);
        let mut asked = Vec::new();
        let r = prewarm_within(&e, Duration::from_millis(200), || clock.get(), |_, n| {
            clock.set(clock.get() + Duration::from_millis(100));
            asked.push(n.to_string());
            n != "n1"
        });
        assert_eq!(r, (1, 1, 2));
        assert_eq!(asked, vec!["n0", "n1"]);
        // a spent budget still queries one entry (every turn makes progress)
        assert_eq!(prewarm_within(&e, Duration::ZERO, || Duration::from_secs(9), |_, _| true), (1, 0, 1));
        assert_eq!(prewarm_within(&e, Duration::from_secs(1), || Duration::ZERO, |_, _| true), (5, 0, 5));
    }

    #[test]
    fn a_top_up_runs_on_a_changed_list_without_a_fresh_fork_with_memory_and_within_its_size() {
        let no_read = || -> Option<u64> { panic!("MemAvailable read without need") };
        assert_eq!(topup_turn(false, false, false, 0, no_read), TopupTurn::Nothing);
        assert_eq!(topup_turn(true, false, true, 0, no_read), TopupTurn::Later);
        assert_eq!(topup_turn(false, true, true, 0, no_read), TopupTurn::Later);
        assert_eq!(topup_turn(true, false, false, 0, || Some(TOPUP_FREE_MB)), TopupTurn::Go);
        assert_eq!(topup_turn(false, true, false, 0, || Some(9000)), TopupTurn::Go);
        assert_eq!(topup_turn(true, false, false, 0, || Some(TOPUP_FREE_MB - 1)), TopupTurn::Wait(TOPUP_FREE_MB - 1));
        assert_eq!(topup_turn(true, false, false, 0, || None), TopupTurn::Wait(0));
        assert_eq!(topup_turn(true, false, false, TOPUP_MAX_MB * 1024 - 1, || Some(9000)), TopupTurn::Go);
        assert_eq!(topup_turn(true, false, false, TOPUP_MAX_MB * 1024, no_read), TopupTurn::Stop);
    }

    #[test]
    fn variant_families_strip_only_a_trailing_v_number() {
        assert_eq!(variant_family("level_02_day_br_v3"), "level_02_day_br");
        assert_eq!(variant_family("level_02_day_br_v12"), "level_02_day_br");
        assert_eq!(variant_family("level_02_day_br"), "level_02_day_br");
        assert_eq!(variant_family("level_02_day_slr_vn"), "level_02_day_slr_vn");
        assert_eq!(variant_family("pve_01_v1_maze1"), "pve_01_v1_maze1");
        assert_eq!(variant_family("level_02_day_br_v"), "level_02_day_br_v");
        assert_eq!(variant_family("level_10_day_slr_props_0"), "level_10_day_slr_props_0");
    }

    #[test]
    fn the_union_keeps_the_old_order_and_adds_only_new_entries() {
        let old = "0x1f\ta.player\n0x31\tanim\n";
        let (t, n, added) = prewarm_union(old, "0x31\tanim\n0xe\titem/x\n0x1f\ta.player\n0xe\titem/x\n");
        assert_eq!(t, "0x1f\ta.player\n0x31\tanim\n0xe\titem/x\n");
        assert_eq!((n, added), (3, 1));
        // nothing new: the same text, nothing added
        assert_eq!(prewarm_union(&t, old), (t.clone(), 3, 0));
        // an empty or missing old list: the new list as it is; lines that are no entries are dropped
        assert_eq!(prewarm_union("", "0x13\tm.model/render\r\nbad line\n0xzz\tq\n0x3\t\n"), ("0x13\tm.model/render\n".to_string(), 1, 1));
        // the same (class, name) in another spelling of the class is one entry; the same name in another class is not
        assert_eq!(prewarm_union("0x1F\ta\n", "0x1f\ta\n0x31\ta\n"), ("0x1f\ta\n0x31\ta\n".to_string(), 2, 1));
    }

    #[test]
    fn a_list_grows_as_a_union_and_is_never_written_empty() {
        let dir = std::env::temp_dir().join(format!("wh-grow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("level_02_day_br_v2.tsv");
        assert_eq!(grow_prewarm_list(&p, "").unwrap(), (0, 0));
        assert!(!p.exists(), "no empty list: the project keeps its sibling fallback");
        assert_eq!(grow_prewarm_list(&p, "0x1f\ta\n0x31\tb\n").unwrap(), (2, 2));
        assert_eq!(grow_prewarm_list(&p, "0x31\tb\n0xe\tc\n").unwrap(), (3, 1));
        assert_eq!(grow_prewarm_list(&p, "0x1f\ta\n").unwrap(), (3, 0));
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "0x1f\ta\n0x31\tb\n0xe\tc\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_takes_in_its_best_variants_list_before_its_zygote_starts() {
        let dir = std::env::temp_dir().join(format!("wh-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("level_02_day_br_v3.tsv"), "0x1f\ta\n0x31\tb\n").unwrap();
        std::fs::write(dir.join("level_02_day_br_v4.tsv"), "0x1f\ta\n0x31\tb\n0xe\tc\n").unwrap();
        let mut t = format!("exe = a\nexe_win = b\nprewarm_dir = {}\n", dir.display());
        for (name, level) in [("level_02_day_br_v2", "level_02/day_br_v2"), ("level_02_day_br_v3", "level_02/day_br_v3"), ("level_02_day_br_v4", "level_02/day_br_v4")] {
            t.push_str(&format!("[project {name}]\nlevel = {level}\n"));
        }
        t.push_str("[project level_02_day_br_v5]\nlevel = level_02/day_br_v5\nprewarm = /pinned.tsv\n");
        let c = DaemonCfg::parse(&t).unwrap();
        let p = |n: &str| c.project(n).unwrap();
        let entries = |n: &str| count_entries(&dir.join(format!("{n}.tsv")));
        // no own list: it starts as the best variant's
        let l = c.seed_prewarm(p("level_02_day_br_v2")).unwrap();
        assert!(l.contains("took in variant level_02_day_br_v4's list: 3 entries, 3 new"), "{l}");
        assert_eq!(c.prewarm_pick(p("level_02_day_br_v2")).source, PrewarmSource::Own);
        assert_eq!(c.seed_prewarm(p("level_02_day_br_v2")), None, "nothing new the second time");
        // a smaller own list takes in what it lacks, keeps its own order
        let l = c.seed_prewarm(p("level_02_day_br_v3")).unwrap();
        assert!(l.contains("3 entries, 1 new"), "{l}");
        assert_eq!(std::fs::read_to_string(dir.join("level_02_day_br_v3.tsv")).unwrap(), "0x1f\ta\n0x31\tb\n0xe\tc\n");
        assert_eq!(c.prewarm_pick(p("level_02_day_br_v3")).source, PrewarmSource::Own);
        // the largest list gains nothing; a configured list is never touched
        assert_eq!(c.seed_prewarm(p("level_02_day_br_v4")), None);
        assert_eq!(entries("level_02_day_br_v4"), 3);
        assert_eq!(c.seed_prewarm(p("level_02_day_br_v5")), None);
        assert!(!dir.join("level_02_day_br_v5.tsv").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_without_any_list_starts_from_the_entries_common_to_the_lists_of_its_kind() {
        let dir = std::env::temp_dir().join(format!("wh-common-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("level_01_day_tdm.tsv"), "0x1f\ta\n0x1f\tb\n0x31\tc\n0xe\tx\n").unwrap();
        std::fs::write(dir.join("level_03_day_br.tsv"), "0x31\tc\n0x1F\ta\n0x1f\tb\n0xe\ty\n").unwrap();
        std::fs::write(dir.join("level_02_day_br_v3.tsv"), "0x1f\ta\n0x1f\tb\n0x31\tc\n0x1f\ta\n").unwrap();
        std::fs::write(dir.join("pve_01_v1_maze2.tsv"), "0x1f\ta\n0x31\tz\n").unwrap();
        let mut t = format!("exe = a\nexe_win = b\nprewarm_dir = {}\n", dir.display());
        for (name, level) in [
            ("level_01_day_tdm", "level_01/day_tdm"),
            ("level_03_day_br", "level_03/day_br"),
            ("level_02_day_br_v2", "level_02/day_br_v2"),
            ("level_02_day_br_v3", "level_02/day_br_v3"),
            ("level_02_day_tdm", "level_02/day_tdm"),
            ("pve_01_v1_maze1", "pve_01/v1_maze1"),
            ("pve_01_v1_maze2", "pve_01/v1_maze2"),
        ] {
            t.push_str(&format!("[project {name}]\nlevel = {level}\n"));
        }
        t.push_str("[project level_05_day_br]\nlevel = level_05/day_br\nprewarm = /pinned.tsv\n");
        let c = DaemonCfg::parse(&t).unwrap();
        let p = |n: &str| c.project(n).unwrap();
        assert_eq!((project_kind(p("pve_01_v1_maze1")), project_kind(p("level_02_day_tdm"))), ("PvE", "PvP"));
        // the three readable PvP lists (the pinned one is unreadable), in the first one's order, each entry once
        assert_eq!(c.common_seed(p("level_02_day_tdm")), Some((3, "0x1f\ta\n0x1f\tb\n0x31\tc\n".to_string())));
        let (l, n) = c.seed_common(p("level_02_day_tdm")).unwrap();
        assert!(l.contains("common seed of 3 PvP lists, 3 entries (3 in the list, 3 new)"), "{l}");
        assert_eq!(n, Some(3));
        assert_eq!(std::fs::read_to_string(dir.join("level_02_day_tdm.tsv")).unwrap(), "0x1f\ta\n0x1f\tb\n0x31\tc\n");
        let pick = c.prewarm_pick(p("level_02_day_tdm"));
        assert_eq!((pick.source, pick.entries), (PrewarmSource::Own, 3), "the seeded list is the project's own; its matches grow it");
        assert_eq!(PrewarmSource::Common(3).to_string(), "common seed of 3 lists");
        assert_eq!(c.seed_common(p("level_02_day_tdm")), None, "an own list is never seeded again");
        // a variant's list comes first; a configured list is never touched
        assert_eq!(c.seed_common(p("level_02_day_br_v2")), None);
        assert_eq!(c.seed_common(p("level_05_day_br")), None);
        // one PvE list is no common seed: logged, nothing written
        let (l, n) = c.seed_common(p("pve_01_v1_maze1")).unwrap();
        assert!(l.contains("no common seed (fewer than 2 PvE lists)") && n.is_none(), "{l}");
        assert!(!dir.join("pve_01_v1_maze1.tsv").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_without_its_own_list_takes_the_largest_list_of_its_variants() {
        let dir = std::env::temp_dir().join(format!("wh-sibling-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("level_02_day_br_v3.tsv"), "0x1f\ta\n0x31\tb\n").unwrap();
        std::fs::write(dir.join("level_02_day_br_v4.tsv"), "0x1f\ta\n0x31\tb\n0xe\tc\n").unwrap();
        std::fs::write(dir.join("level_01_day_tdm.tsv"), "0x1f\ta\n").unwrap();
        std::fs::write(dir.join("level_01_day_tdm_v2.tsv"), "").unwrap();
        std::fs::write(dir.join("pve_01_v1_maze2.tsv"), "0x1f\ta\n").unwrap();
        let mut t = format!("exe = a\nexe_win = b\nprewarm_dir = {}\n", dir.display());
        for (name, level) in [
            ("level_02_day_br", "level_02/day_br"),
            ("level_02_day_br_v2", "level_02/day_br_v2"),
            ("level_02_day_br_v3", "level_02/day_br_v3"),
            ("level_02_day_br_v4", "level_02/day_br_v4"),
            ("level_02_day_tdm", "level_02/day_tdm"),
            ("level_01_day_tdm", "level_01/day_tdm"),
            ("level_01_day_tdm_v1", "level_01/day_tdm_v1"),
            ("level_01_day_tdm_v2", "level_01/day_tdm_v2"),
            ("level_03_day_br_v1", "level_03/day_br_v1"),
            ("pve_01_v1_maze1", "pve_01/v1_maze1"),
            ("pve_01_v1_maze2", "pve_01/v1_maze2"),
        ] {
            t.push_str(&format!("[project {name}]\nlevel = {level}\n"));
        }
        t.push_str("[project level_03_day_br]\nlevel = level_03/day_br\nprewarm = /pinned.tsv\n");
        let c = DaemonCfg::parse(&t).unwrap();
        let pick = |n: &str| c.prewarm_pick(c.project(n).unwrap());
        let rec = |n: &str| Some(dir.join(format!("{n}.tsv")));

        // an own list with at least as many entries as the best variant's
        let own = pick("level_02_day_br_v4");
        assert_eq!(own, PrewarmPick { source: PrewarmSource::Own, list: rec("level_02_day_br_v4"), entries: 3, record: rec("level_02_day_br_v4") });
        // an own list smaller than a variant's (its first matches only began it): the variant's, still recording its own
        let v3 = pick("level_02_day_br_v3");
        assert_eq!(v3, PrewarmPick { source: PrewarmSource::Sibling("level_02_day_br_v4".into()), list: rec("level_02_day_br_v4"), entries: 3, record: rec("level_02_day_br_v3") });
        // no own list: the variant with the most entries; the project still records into its own list
        let v2 = pick("level_02_day_br_v2");
        assert_eq!(v2, PrewarmPick { source: PrewarmSource::Sibling("level_02_day_br_v4".into()), list: rec("level_02_day_br_v4"), entries: 3, record: rec("level_02_day_br_v2") });
        assert_eq!(v2.source.to_string(), "sibling level_02_day_br_v4");
        assert_eq!(pick("level_02_day_br").source, PrewarmSource::Sibling("level_02_day_br_v4".into()), "the base name is a variant too");
        assert_eq!(pick("level_01_day_tdm_v1").source, PrewarmSource::Sibling("level_01_day_tdm".into()));
        // an empty own list counts as none
        assert_eq!(pick("level_01_day_tdm_v2").source, PrewarmSource::Sibling("level_01_day_tdm".into()));
        // another mode of the same level, the same mode of another level, another maze: no fallback
        assert_eq!(pick("level_02_day_tdm"), PrewarmPick { source: PrewarmSource::None, list: None, entries: 0, record: rec("level_02_day_tdm") });
        assert_eq!(pick("level_02_day_tdm").source.to_string(), "none");
        assert_eq!(pick("pve_01_v1_maze1").source, PrewarmSource::None);
        // a configured list is the project's own even when absent (the zygote reports the error), and is never grown;
        // its variant without a list of its own does not fall back to it: unreadable lists have no entries
        assert_eq!(pick("level_03_day_br"), PrewarmPick { source: PrewarmSource::Own, list: Some(PathBuf::from("/pinned.tsv")), entries: 0, record: None });
        assert_eq!(pick("level_03_day_br_v1").source, PrewarmSource::None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_relays_closing_dfs_are_the_four_before_the_close_tick() {
        let dfs: Vec<(u32, ())> = (0..10).map(|i| (1000 + i * 50, ())).collect(); // 1000 .. 1450
        assert_eq!(fin_dfs(Some(1500), 50, &dfs), 4);
        assert_eq!(fin_dfs(Some(1700), 50, &dfs), 0); // a close without the closing DFs
        assert_eq!(fin_dfs(None, 50, &dfs), 0);
        assert_eq!(fin_dfs(Some(1500), 50, &dfs[..8]), 2);
    }

    #[test]
    fn a_handed_over_frame_parses_back() {
        let f = world_host_proto::ToHost::Close { t_last: 7 }.frame().unwrap();
        let p = parse_frame(&f).unwrap();
        assert!(matches!(ToHost::decode(&p), Ok(ToHost::Close { t_last: 7 })));
        let mut two = f.clone();
        two.extend_from_slice(&f);
        assert!(parse_frame(&two).is_err());
        assert!(parse_frame(&f[..f.len() - 1]).is_err());
    }

    fn sv(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn a_zygotes_command_line_is_the_one_it_always_had() {
        let c = DaemonCfg::parse("exe = /g/survarium.exe\nexe_win = C:\\G\\survarium.exe\nmax_matches = 3\nidle_secs = 600\ntick_frames = 0\n[project pve_01_v1_maze2]\nlevel = pve_01/v1_maze2\n").unwrap();
        let p = c.project("pve_01_v1_maze2").unwrap();
        let none = PrewarmPick { source: PrewarmSource::None, list: None, entries: 0, record: None };
        let out = std::path::Path::new("/o/pve_01_v1_maze2/z1");
        let mut want = sv(&["wh", "--exe", "/g/survarium.exe", "--exe-win", "C:\\G\\survarium.exe", "--out", "/o/pve_01_v1_maze2/z1"]);
        want.extend(sv(HOOK_ARGS));
        want.extend(sv(&["--level", "pve_01/v1_maze2", "--zygote-fd", "3", "--max-matches", "3", "--idle-secs", "600"]));
        want.extend(sv(&["--backstop-min", &c.backstop_min.to_string(), "--read-timeout", &c.read_timeout_secs.to_string()]));
        assert_eq!(zygote_args(&c, p, &none, out, "wh"), want);
        let sib = PrewarmPick { source: PrewarmSource::Sibling("pve_01_v1_maze1".into()), list: Some("/l/m1.tsv".into()), entries: 9, record: Some("/l/m2.tsv".into()) };
        let a = zygote_args(&c, p, &sib, out, "wh");
        assert_eq!(a[..want.len()], want[..]);
        assert_eq!(a[want.len()..], sv(&["--prewarm", "/l/m1.tsv", "--prewarm-source", "sibling pve_01_v1_maze1", "--record-prewarm", "/l/m2.tsv"])[..]);
        let t = DaemonCfg::parse("exe = a\nexe_win = b\ntick_frames = 1\n[project x]\nlevel = l\n").unwrap();
        assert!(crate::cmdline::has_in(&zygote_args(&t, t.project("x").unwrap(), &none, out, "wh"), "--tick-frames"));
    }

    #[test]
    fn evicted_zygotes_are_dropped_when_ended_and_killed_once_when_hung() {
        use std::cell::Cell;
        let t0 = Instant::now();
        // proc_ = (alive, kills)
        let r = |name: &str, alive: bool, since_s: u64, killed: bool| Retired { name: name.into(), pid: 1, proc_: (Cell::new(alive), Cell::new(0u32)), since: t0 + Duration::from_secs(since_s), killed };
        let mut v = vec![r("ended", false, 0, false), r("fresh", true, 5, false), r("hung", true, 0, false), r("killed", true, 0, true)];
        let now = t0 + RETIRE_KILL + Duration::from_secs(1);
        let alive = |p: &(Cell<bool>, Cell<u32>)| p.0.get();
        let kill = |p: &(Cell<bool>, Cell<u32>)| p.1.set(p.1.get() + 1);
        let lines = retire_turn(&mut v, now, alive, kill);
        assert_eq!(v.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["fresh", "hung", "killed"], "an ended one is dropped, the others still count");
        assert_eq!(v.iter().map(|x| x.proc_.1.get()).collect::<Vec<_>>(), [0, 1, 0], "only the hung one is killed, a killed one not again");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("ended") && lines[1].contains("hung"));
        let lines = retire_turn(&mut v, now + Duration::from_secs(1), alive, kill);
        assert!(lines.is_empty());
        assert_eq!(v[1].proc_.1.get(), 1, "killed once");
        v[1].proc_.0.set(false);
        retire_turn(&mut v, now, alive, kill);
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn only_a_readable_or_hung_up_pidfd_means_ended() {
        assert!(!pidfd_ended(0, 0), "no event: alive");
        assert!(!pidfd_ended(-1, 0), "a poll error (EINTR, ENOMEM) says nothing: alive");
        assert!(!pidfd_ended(-1, libc::POLLIN), "a stale revents after an error does not count");
        assert!(pidfd_ended(1, libc::POLLIN));
        assert!(pidfd_ended(1, libc::POLLHUP));
        assert!(!pidfd_ended(1, libc::POLLNVAL), "a bad descriptor is not an ended process");
    }

    #[test]
    fn a_base_is_replaced_after_six_hours_and_base_waits_get_thirty_seconds() {
        let t0 = Instant::now();
        assert!(!base_too_old(t0, t0 + BASE_MAX_AGE - Duration::from_secs(1), BASE_MAX_AGE));
        assert!(base_too_old(t0, t0 + BASE_MAX_AGE, BASE_MAX_AGE));
        assert!(!base_too_old(t0 + Duration::from_secs(5), t0, BASE_MAX_AGE), "a start after `now`");
        // the diagnosis switch: seconds, at least BASE_MIN_AGE_DIAG; unset or garbage = the 6 h
        assert_eq!(base_max_age_of(None), None);
        assert_eq!(base_max_age_of(Some("x")), None);
        assert_eq!(base_max_age_of(Some("600")), Some(Duration::from_secs(600)));
        assert_eq!(base_max_age_of(Some("5")), Some(BASE_MIN_AGE_DIAG));
        assert!(base_too_old(t0, t0 + Duration::from_secs(600), Duration::from_secs(600)));
        for why in [BASE_STARTING, BASE_LOST, BASE_BUSY] {
            assert_eq!(wait_limit(why), BASE_START_WAIT, "{why}");
        }
        assert_eq!(wait_limit("the busy maps fill the limit"), ROOM_WAIT);
        assert_eq!(wait_limit("an evicted map is still exiting"), ROOM_WAIT);
    }

    #[test]
    fn a_new_base_starts_at_most_every_restart_gap() {
        let t0 = Instant::now();
        assert!(base_start_due(None, t0), "the first one at once");
        assert!(!base_start_due(Some(t0), t0 + BASE_RESTART_GAP - Duration::from_millis(1)));
        assert!(base_start_due(Some(t0), t0 + BASE_RESTART_GAP));
        assert!(!base_start_due(Some(t0 + Duration::from_secs(1)), t0), "a start after `now`");
    }

    #[test]
    fn a_match_refused_while_the_base_is_not_ready_says_so() {
        assert_eq!(refusal_text(BASE_STARTING), "the host's base process is not ready");
        assert_eq!(refusal_text(BASE_LOST), "the host's base process is not ready");
        assert_eq!(refusal_text(BASE_BUSY), "the host's base process is not ready");
        assert_eq!(refusal_text("the busy maps fill the limit"), "the host has no room for another map now");
    }

    #[test]
    fn the_base_answers_loaded_or_failed() {
        assert_eq!(base_reply("p", Ok(77)), crate::base::FromBase::Loaded { project: "p".into(), pid: 77 });
        assert_eq!(base_reply("p", Err(&Refusal::Failed("fork: no memory".into()))), crate::base::FromBase::Failed { project: "p".into(), reason: "fork: no memory".into() });
        assert_eq!(base_reply("p", Err(&Refusal::Busy("fork census timeout".into()))), crate::base::FromBase::Busy { project: "p".into(), reason: "fork census timeout".into() });
    }

    #[test]
    fn the_base_gets_the_hook_flags_but_no_level_and_no_zygote_options() {
        let c = DaemonCfg::parse("exe = /g/s.exe\nexe_win = C:\\s.exe\n[project x]\nlevel = l\n").unwrap();
        let a = base_args(&c, std::path::Path::new("/o/base/b1"), "wh", None);
        assert_eq!(crate::cmdline::value_in(&a, "--base-fd").as_deref(), Some("3"));
        assert_eq!(crate::cmdline::value_in(&a, "--out").as_deref(), Some("/o/base/b1"));
        for k in ["--level", "--zygote-fd", "--prewarm", "--max-matches", "--base-prewarm", "--base-exclude"] {
            assert!(!crate::cmdline::has_in(&a, k), "{k}");
        }
        // with a base pre-warm set: its file and the exclusion file (in prewarm_dir, else in out)
        let s = base_args(&c, std::path::Path::new("/o/base/b1"), "wh", Some(std::path::Path::new("/o/base/b1/base-prewarm.tsv")));
        assert_eq!(crate::cmdline::value_in(&s, "--base-prewarm").as_deref(), Some("/o/base/b1/base-prewarm.tsv"));
        assert_eq!(crate::cmdline::value_in(&s, "--base-exclude").as_deref(), Some("/var/lib/srp-world-host/base-exclude.tsv"));
        assert_eq!(s[..a.len()], a[..], "the same command line before them");
        let d = DaemonCfg::parse("exe = /g/s.exe\nexe_win = C:\\s.exe\nprewarm_dir = /pw\n[project x]\nlevel = l\n").unwrap();
        assert_eq!(d.base_exclude_path(), PathBuf::from("/pw/base-exclude.tsv"));
        let z = zygote_args(&c, c.project("x").unwrap(), &PrewarmPick { source: PrewarmSource::None, list: None, entries: 0, record: None }, std::path::Path::new("/z"), "wh");
        assert_eq!(a[7..7 + HOOK_ARGS.len()], z[7..7 + HOOK_ARGS.len()], "the same hook flags as every zygote");
    }

    #[test]
    fn the_room_status_reader_prints_every_map_and_the_drain() {
        use world_host_proto::status::{HostStatus, LoadKind, LoadedMap, MatchRoom, NewMap, NewMaps};
        let map = |p: &str, running, evict, room| LoadedMap { project: p.into(), running, starting: 0, room, evict, loaded_secs: 61 };
        let mut st = HostStatus {
            max_zygotes: Some(2),
            max_matches: 2,
            free_mb: Some(3000),
            new_maps: NewMaps::Room(vec![NewMap { kind: LoadKind::Over, evicts: 1 }]),
            loaded: vec![map("a", 2, None, MatchRoom::Over), map("b", 0, Some(0), MatchRoom::Within)],
            drain: None,
        };
        let t = room_status_text(&st, 1.0);
        let lines: Vec<&str> = t.lines().collect();
        assert!(lines[0].starts_with("1.0 s: room status: 2 loaded (2 with room, 1 idle, 2 matches), max_zygotes 2, max_matches 2, new maps: 1 can load"), "{}", lines[0]);
        assert_eq!(lines[1], "  map a: running 2, starting 0, room Over, busy, loaded 61 s");
        assert_eq!(lines[2], "  map b: running 0, starting 0, room Within, idle (eviction rank 0), loaded 61 s");
        assert_eq!(lines[3], "  new map 1: Over, evicts 1");
        assert_eq!(lines.len(), 4);
        st.new_maps = NewMaps::NoRoom;
        st.drain = Some("a".into());
        let t = room_status_text(&st, 2.0);
        assert!(t.ends_with("  new maps: none can load\n  drain: a\n"), "{t}");
        assert!(t.lines().next().unwrap().ends_with(", draining a"));
    }
}
