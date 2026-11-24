//! `srp-world-host`: the world host daemon.
//!
//! Three kinds of process, all this binary:
//! * **front** (`world-host daemon --config FILE`): listens on the unix socket, reads each connection's first frame
//!   (`Create`), and hands the connection (descriptor + the frame) to the zygote of the match's level project,
//!   starting that zygote on first use. It never loads the exe.
//! * **zygote** (started by the front with `--zygote-fd 3`): boots the exe, loads one level project with the null
//!   render and the null cooks, queries its pre-warm list, then waits for hand-overs and `fork()`s one child per match.
//!   It never creates a match itself (one match per process: the core is the static 0x20F442760).
//! * **child**: takes over the exe threads (`run::after_fork`), creates the match from `Create` (client mode,
//!   [`crate::world::MatchWorld`]), answers `Ready`, then runs [`crate::session::Session`] on the connection until
//!   `Close`, a fault or EOF (EOF = the relay is gone: the child exits). One connection = one match: the child's exit
//!   is the relay's EOF.
//!
//! Limits: at most `max_matches` children per zygote (more are refused with `Fault`); at most `max_zygotes` live
//! zygotes (0 = no cap): starting one more first retires the least recently used ones (not the `prestart` ones) by
//! closing their control socket, which makes a zygote exit once its running matches ended (they keep sharing its
//! pages until then; a zygote is about 0.35..0.56 GB RSS); a child's
//! `oom_score_adj` is 500 (an OOM kills a match, not the zygote); a child has a wall-clock backstop (SIGALRM) and a
//! read timeout; an idle zygote exits after `idle_secs` and is started again on demand.

use crate::hlog;
use crate::session::{Session, SessionConfig};
use std::collections::HashMap;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
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
    /// pre-warm lists: a project without its own `prewarm` uses `<prewarm_dir>/<project>.tsv` when that file exists;
    /// when it does not, the project's zygote records it at its first match
    pub prewarm_dir: Option<PathBuf>,
    /// projects whose zygote starts with the front (the others start on their first match)
    pub prestart: Vec<String>,
    /// live zygotes at most (0 = no cap): above it the least recently used non-prestart zygotes are retired
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

    /// The project's pre-warm list: its own `prewarm`, else the shipped `<prewarm_dir>/<name>.tsv` if present.
    pub fn prewarm_of(&self, p: &ProjectCfg) -> Option<PathBuf> {
        p.prewarm.clone().or_else(|| self.prewarm_dir.as_ref().map(|d| d.join(format!("{}.tsv", p.name))).filter(|f| f.is_file()))
    }
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
    child: Child,
    ctl: UnixStream,
    /// the last match handed to it (its start for a new one)
    last_used: Instant,
}

/// The zygotes to retire before one more starts under a cap of `cap` live zygotes (0 = no cap): the least recently used
/// first, never a `keep` one (the prestart zygotes), until the new one fits. `live` = (project, last match handed over).
fn lru_victims(live: &[(String, Instant)], keep: &[String], cap: u32) -> Vec<String> {
    if cap == 0 {
        return Vec::new();
    }
    let mut cands: Vec<&(String, Instant)> = live.iter().filter(|(n, _)| !keep.contains(n)).collect();
    cands.sort_by_key(|(_, t)| *t);
    let over = (live.len() + 1).saturating_sub(cap as usize);
    cands.into_iter().take(over).map(|(n, _)| n.clone()).collect()
}

/// Zygotes the front retired (control socket closed): reaped once they exited.
static RETIRED: Mutex<Vec<(String, Child)>> = Mutex::new(Vec::new());

/// Reap retired zygotes that exited; the ones still waiting for their matches stay listed.
fn reap_retired() {
    RETIRED.lock().unwrap().retain_mut(|(name, c)| match c.try_wait() {
        Ok(None) => true,
        Ok(Some(st)) => {
            hlog!("front: retired zygote {name} (pid {}) exited: {st}", c.id());
            false
        }
        Err(_) => false,
    });
}

/// Make room for one more zygote under `cfg.max_zygotes`: drop the dead entries, then retire the least recently used.
fn make_room(cfg: &DaemonCfg, z: &mut HashMap<String, Zygote>) {
    z.retain(|_, x| matches!(x.child.try_wait(), Ok(None)));
    let live: Vec<(String, Instant)> = z.iter().map(|(n, x)| (n.clone(), x.last_used)).collect();
    for name in lru_victims(&live, &cfg.prestart, cfg.max_zygotes) {
        if let Some(old) = z.remove(&name) {
            hlog!(
                "front: {} live zygotes reach max_zygotes {}: retiring {name} (pid {}, last match {} s ago); it exits when its matches ended",
                live.len() + 1,
                cfg.max_zygotes,
                old.child.id(),
                old.last_used.elapsed().as_secs()
            );
            // dropping the control socket ends the zygote's loop (zygote_loop: Ok(None))
            let Zygote { child, ctl, .. } = old;
            drop(ctl);
            RETIRED.lock().unwrap().push((name, child));
        }
    }
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
    for name in &cfg.prestart {
        let p = cfg.project(name).expect("checked by the config parser");
        match spawn_zygote(&cfg, p) {
            Ok(z) => {
                zygotes.lock().unwrap().insert(p.name.clone(), z);
            }
            Err(e) => hlog!("front: prestart {name}: {e}"),
        }
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
    reap_retired();
    // one retry with a fresh zygote when the old one is gone
    for attempt in 0..2 {
        let mut z = zygotes.lock().unwrap();
        let alive = z.get_mut(&proj.name).is_some_and(|x| matches!(x.child.try_wait(), Ok(None)));
        if !alive {
            z.remove(&proj.name);
            make_room(cfg, &mut z);
            match spawn_zygote(cfg, proj) {
                Ok(n) => {
                    z.insert(proj.name.clone(), n);
                }
                Err(e) => {
                    hlog!("front: zygote {}: {e}", proj.name);
                    let _ = s.write_all(&fault_frame(&format!("zygote: {e}")));
                    return;
                }
            }
        }
        let zy = z.get_mut(&proj.name).expect("inserted");
        match crate::fdpass::send_fd(&zy.ctl, s.as_raw_fd(), &bytes) {
            Ok(()) => {
                zy.last_used = Instant::now();
                hlog!("front: match {:#x} ({}) handed to zygote pid {}", create.match_id, proj.name, zy.child.id());
                return;
            }
            Err(e) => {
                hlog!("front: hand-over to zygote {} failed (attempt {attempt}): {e}", proj.name);
                if let Some(mut old) = z.remove(&proj.name) {
                    let _ = old.child.kill();
                    let _ = old.child.wait();
                }
            }
        }
    }
    let _ = s.write_all(&fault_frame("no zygote took the match"));
}

/// Start a zygote for one project: this binary with the host's fixed hook flags, the level and the control socket
/// as descriptor 3.
fn spawn_zygote(cfg: &DaemonCfg, p: &ProjectCfg) -> io::Result<Zygote> {
    let (ours, theirs) = UnixStream::pair()?;
    // one folder per zygote (its log, private drive and match folders): a restarted front's new zygote never shares
    // files with an old one that still waits for its matches
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let out = cfg.out.join(&p.name).join(format!("z{ms}"));
    std::fs::create_dir_all(&out)?;
    let mut cmd = Command::new(std::env::current_exe()?);
    cmd.args(["--exe", &cfg.exe.to_string_lossy(), "--exe-win", &cfg.exe_win, "--out", &out.to_string_lossy()]);
    cmd.args(["--steps", "3", "--render-world", "--null-render", "--null-cooks", "6,d,7b,render", "--hide-visuals"]);
    cmd.args(["--escape-at", "USER32.dll!LoadIconA", "--end-threads", ":network", "--quiet", "--level", &p.level]);
    cmd.args(["--zygote-fd", "3", "--max-matches", &cfg.max_matches.to_string(), "--idle-secs", &cfg.idle_secs.to_string()]);
    cmd.args(["--backstop-min", &cfg.backstop_min.to_string(), "--read-timeout", &cfg.read_timeout_secs.to_string()]);
    if cfg.tick_frames {
        cmd.arg("--tick-frames");
    }
    match (cfg.prewarm_of(p), &cfg.prewarm_dir) {
        (Some(w), _) => {
            cmd.args(["--prewarm", &w.to_string_lossy()]);
        }
        // no list yet: this zygote logs the resource queries and its first match writes the list (bp::record_prewarm)
        (None, Some(d)) => {
            cmd.args(["--record-prewarm", &d.join(format!("{}.tsv", p.name)).to_string_lossy()]);
        }
        (None, None) => {}
    }
    cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let fd = theirs.as_raw_fd();
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(move || {
            if libc::dup2(fd, 3) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    drop(theirs);
    hlog!("front: zygote for {} ({}) started, pid {}", p.name, p.level, child.id());
    Ok(Zygote { child, ctl: ours, last_used: Instant::now() })
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
        let a: Vec<String> = std::env::args().collect();
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

/// The zygote's loop, on the world thread after the level (and the pre-warm list) loaded. Returns when the front
/// closed the control socket or the zygote was idle for `idle_secs`; a child never returns from it.
pub fn zygote_loop(o: &ZygoteOpts) -> Result<(), String> {
    let ctl = unsafe { UnixStream::from_raw_fd(o.ctl_fd) };
    let mut children: Vec<(u32, i32, Instant)> = Vec::new();
    let mut idle_since = Instant::now();
    crate::run::mem_snapshot("zygote ready");
    if std::env::var("HOST_MEM_BREAKDOWN").is_ok() {
        crate::run::mem_breakdown("zygote ready");
    }
    hlog!("zygote: ready (max {} matches)", o.max_matches);
    crate::log::flush();
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
        if !children.is_empty() {
            idle_since = Instant::now();
        } else if idle_since.elapsed().as_secs() >= o.idle_secs {
            hlog!("zygote: idle for {} s, exiting", o.idle_secs);
            return Ok(());
        }
        let mut pfd = libc::pollfd { fd: ctl.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        if unsafe { libc::poll(&mut pfd, 1, 1000) } <= 0 {
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
                hlog!("zygote: hand-over: {e}");
                continue;
            }
        };
        let mut s = UnixStream::from(conn);
        if children.len() as u32 >= o.max_matches {
            hlog!("zygote: {} matches running, refusing another", children.len());
            let _ = s.write_all(&fault_frame("the host runs its maximum number of matches"));
            continue;
        }
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
        crate::log::flush();
        let pid = crate::run::fork_world();
        if pid == 0 {
            drop(ctl);
            let code = child_main(s, frame, o);
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
fn child_main(mut s: UnixStream, create: Frame, o: &ZygoteOpts) -> i32 {
    let c = match ToHost::decode(&create) {
        Ok(ToHost::Create(c)) => c,
        _ => return 2,
    };
    // match id + pid: a relay may retry one match id, and test recordings share theirs
    let dir = PathBuf::from(crate::run::out_dir()).join(format!("m{:08x}-{}", c.match_id, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::log::open(dir.join("host.log").to_str().unwrap_or("host.log"));
    hlog!("child {}: match {:#x}, project '{}', kind {}, {} records", std::process::id(), c.match_id, c.project, c.kind, c.records.len());
    let _ = std::fs::write("/proc/self/oom_score_adj", "500");
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
            hlog!("child: done after {} steps; render commands retired {} ({} with the consumer's keep marks), render pool peak {} of 8192", sess.steps, mw.render_retired, mw.render_marked, mw.render_pool_peak);
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

    #[test]
    fn the_least_recently_used_zygotes_are_retired_above_the_cap() {
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let live: Vec<(String, Instant)> = [("pve", 0), ("a", 5), ("b", 1), ("c", 9)].iter().map(|(n, s)| (n.to_string(), at(*s))).collect();
        let keep = vec!["pve".to_string()];
        assert!(lru_victims(&live, &keep, 0).is_empty(), "no cap");
        assert!(lru_victims(&live, &keep, 5).is_empty(), "4 live + the new one fit in 5");
        assert_eq!(lru_victims(&live, &keep, 4), ["b"], "the oldest non-prestart one makes room");
        assert_eq!(lru_victims(&live, &keep, 3), ["b", "a"]);
        assert_eq!(lru_victims(&live, &keep, 1), ["b", "a", "c"], "the prestart zygote is never retired");
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
}
