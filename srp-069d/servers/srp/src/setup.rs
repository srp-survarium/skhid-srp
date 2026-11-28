//! `srp setup`: check this computer for a 0.69d server and prepare everything a first start needs, in plain words.
//!
//! Checks: the system (Linux or WSL2), the game folder, the exe build (size + SHA-256 of the 0.69d build the world
//! host's addresses belong to: other builds are refused), the game's resource files, the world host program, the ports,
//! memory and disk. Then: writes a commented config (`config/srp.example.toml` with the game folder and the addresses
//! filled in) when there is none, creates the database, generates the login key when it is missing, and prints the
//! line players start the game with.
//!
//! The same exe check guards `srp check`, `srp run` and `srp world-host` ([`check_exe`]).

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::path::{Path, PathBuf};

use login_server::LoginKey;

use crate::settings::{self, EXAMPLE};
use crate::Config;

/// Size of the 0.69d `survarium.exe` (the last Steam build of Survarium, February 2022) whose code the world host runs.
pub const EXE_SIZE: u64 = 13_386_392;
/// SHA-256 of that exe. The world host calls the exe's code at fixed addresses, so only this exact file works.
pub const EXE_SHA256: &str = "1e1ae04dc981fc4f5734c56559fb0f2e0e74c787bcc53ebbfdc233f440d65448";
/// Memory one loaded map ("zygote") takes on its own, the largest kind, counted without the base pre-warm set's
/// saving: the largest map (one level of three modes, the level alone 0.28 GB) with a pre-warm list measures
/// 0.32-0.34 GB in one-shot runs and 308 MB as a zygote in a played match (kept at 0.35), a PvE mission zygote
/// 0.25-0.26 GB (0.2 with the set), a usual PvP map 0.11-0.15 GB (61 MB measured with the set). Zygotes are forks of the world host's base
/// process and share its start-up pages (docs/guide-server.md, "Memory: how many maps and matches").
const ZYGOTE_MB: u64 = 350;
/// One running match, the larger kind. Measured maxima: a 12-slot PvP match with 11 AI players on a map started from
/// the common seed 137-140.5 MB, a PvE mission with the host's AI driving 17 AI players 122 MB, an 18-minute PvE
/// mission 106 MB, a 16-slot PvP match with 15 AI players (level-1 equipment) 96-117 MB. Kept above them: higher
/// equipment levels make a match larger.
const MATCH_MB: u64 = 165;
/// srp + the world host's front and base processes (measured: srp about 40 MB, the base 124 MB at the cut and about
/// 168 MB with a 1413-entry base pre-warm set; the set grows with more lists and a higher max_zygotes, and a replaced
/// base's pages stay with its maps for a while). Kept: the set's size at the default max_zygotes is not measured.
const BASE_MB: u64 = 350;
/// A usual PvP map once its list's common part sits in the base process (measured 61-68 MB; 101-116 MB without it,
/// levels alone 31-98 MB) and a usual PvP match: filled with AI players (the default settings fill a match short of
/// players after bot_wait_secs; 11 AI players in 12 slots measured 0.13-0.14 GB): the typical-PvP fit.
const TYPICAL_PVP_MAP_MB: u64 = 85;
const TYPICAL_PVP_MATCH_MB: u64 = 130;

/// The most the server can use with `max_zygotes` maps of `max_matches` matches each (the guide's formula, larger
/// kinds), in MB; None for `max_zygotes` 0 (no cap).
pub fn memory_need_mb(max_zygotes: u32, max_matches: u32) -> Option<u64> {
    (max_zygotes != 0).then(|| BASE_MB + u64::from(max_zygotes) * (ZYGOTE_MB + u64::from(max_matches) * MATCH_MB))
}

/// The `max_zygotes` that fits `available_mb` with `max_matches` matches per map (at least 1).
pub fn zygotes_that_fit(available_mb: u64, max_matches: u32) -> u32 {
    let per_map = ZYGOTE_MB + u64::from(max_matches) * MATCH_MB;
    (available_mb.saturating_sub(BASE_MB) / per_map).max(1) as u32
}

/// The usual PvP maps with `max_matches` usual matches each that fit `available_mb` (at least 1): what a server
/// that mostly runs PvP maps can expect, next to the worst case of [`zygotes_that_fit`].
pub fn typical_pvp_maps_that_fit(available_mb: u64, max_matches: u32) -> u32 {
    let per_map = TYPICAL_PVP_MAP_MB + u64::from(max_matches) * TYPICAL_PVP_MATCH_MB;
    (available_mb.saturating_sub(BASE_MB) / per_map).max(1) as u32
}

/// The two fits `srp setup` prints for `available_mb`: the worst case and the usual PvP maps.
pub fn memory_fit_text(available_mb: u64, max_matches: u32) -> String {
    format!(
        "this memory fits {} map(s) in the worst case (each map its largest kind, every match slot used) or about {} usual PvP maps, with max_matches = {max_matches}",
        zygotes_that_fit(available_mb, max_matches),
        typical_pvp_maps_that_fit(available_mb, max_matches)
    )
}
/// Free disk below this gets a warning (logs, the database, the world host's per-match folders).
const DISK_WARN_MB: u64 = 2048;

/// SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
        0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
        0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
        0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let bits = (data.len() as u64).wrapping_mul(8);
    let mut tail = data[data.len() - data.len() % 64..].to_vec();
    tail.push(0x80);
    while tail.len() % 64 != 56 {
        tail.push(0);
    }
    tail.extend(bits.to_be_bytes());
    for block in data[..data.len() - data.len() % 64].chunks_exact(64).chain(tail.chunks_exact(64)) {
        let mut w = [0u32; 64];
        for (i, c) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `13,386,392`
fn grouped(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Is `exe` the 0.69d build the world host needs? Err = a message for the operator.
pub fn check_exe(exe: &Path) -> Result<(), String> {
    let mut f = std::fs::File::open(exe).map_err(|e| format!("cannot read {}: {e}", exe.display()))?;
    let mut data = Vec::new();
    f.read_to_end(&mut data).map_err(|e| format!("cannot read {}: {e}", exe.display()))?;
    let sum = hex(&sha256(&data));
    if data.len() as u64 == EXE_SIZE && sum == EXE_SHA256 {
        return Ok(());
    }
    Err(format!(
        "{} is not the Survarium 0.69d build this server needs: it has {} bytes and SHA-256 {sum}; the 0.69d build has {} bytes and \
         SHA-256 {EXE_SHA256}. The world host runs the game's own code at fixed addresses, so only this exact build works \
         (the last Steam build of Survarium, February 2022). Use an unmodified 0.69d install.",
        exe.display(),
        grouped(data.len() as u64),
        grouped(EXE_SIZE)
    ))
}

/// A game install: the game folder (the one with `resources.NN.db` and `binaries/`) and its exe.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub root: PathBuf,
    pub exe: PathBuf,
}

/// The install at `p`: the exe itself, the game folder, the Steam `Survarium` folder (holding `game/`), or the
/// `binaries/x64` folder.
pub fn game_at(p: &Path) -> Option<Game> {
    let exe = if p.is_file() {
        p.to_path_buf()
    } else {
        ["binaries/x64/survarium.exe", "game/binaries/x64/survarium.exe", "survarium.exe"].iter().map(|r| p.join(r)).find(|e| e.is_file())?
    };
    let exe = std::path::absolute(&exe).unwrap_or(exe);
    let root = exe.parent()?.parent()?.parent()?.to_path_buf();
    Some(Game { root, exe })
}

/// Steam library folders named in Steam's `libraryfolders.vdf` text (`"path"  "D:\\SteamLibrary"`), as Linux paths: a
/// Windows path becomes its WSL mount (`/mnt/d/SteamLibrary`).
pub fn steam_libraries(vdf: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for line in vdf.lines() {
        let t = line.trim();
        let Some(rest) = t.strip_prefix("\"path\"") else { continue };
        let v = rest.trim().trim_matches('"').replace("\\\\", "\\");
        let b = v.as_bytes();
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            let tail = v[2..].replace('\\', "/");
            out.push(PathBuf::from(format!("/mnt/{}{}", (b[0] as char).to_ascii_lowercase(), if tail.starts_with('/') { tail } else { format!("/{tail}") })));
        } else if v.starts_with('/') {
            out.push(PathBuf::from(v));
        }
    }
    out
}

/// Where a Steam install of Survarium usually is (Windows Steam seen from WSL, Steam on Linux, its library folders).
pub fn search_game() -> Option<Game> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut steams: Vec<PathBuf> = Vec::new();
    for d in b'a'..=b'z' {
        let m = PathBuf::from(format!("/mnt/{}", d as char));
        if m.is_dir() {
            steams.extend([m.join("Program Files (x86)/Steam"), m.join("Program Files/Steam"), m.join("Steam"), m.join("SteamLibrary"), m.join("Games/Steam")]);
        }
    }
    steams.extend([home.join(".steam/steam"), home.join(".local/share/Steam")]);
    let mut libs = steams.clone();
    for s in &steams {
        if let Ok(t) = std::fs::read_to_string(s.join("steamapps/libraryfolders.vdf")) {
            libs.extend(steam_libraries(&t));
        }
    }
    libs.iter().find_map(|l| game_at(&l.join("steamapps/common/Survarium")))
}

/// The game's resource files: `resources.NN.db` in the game folder, each readable. Ok = how many.
pub fn check_resources(root: &Path) -> Result<usize, String> {
    let rd = std::fs::read_dir(root).map_err(|e| format!("cannot list {}: {e}", root.display()))?;
    let mut n = 0;
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_lowercase();
        let Some(num) = name.strip_prefix("resources.").and_then(|r| r.strip_suffix(".db")) else { continue };
        if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let mut buf = [0u8; 4096];
        std::fs::File::open(e.path())
            .and_then(|mut f| f.read(&mut buf))
            .map_err(|err| format!("cannot read {}: {err} (check the file permissions)", e.path().display()))?;
        n += 1;
    }
    if n == 0 {
        return Err(format!("no resources.NN.db files in {}: this is not the game folder, or the install is incomplete", root.display()));
    }
    if !root.join("resources").is_dir() {
        return Err(format!("no resources folder in {}: the install is incomplete", root.display()));
    }
    Ok(n)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Platform {
    Linux,
    Wsl2,
    Wsl1,
    Other,
}

pub fn platform() -> Platform {
    if !cfg!(target_os = "linux") {
        return Platform::Other;
    }
    let v = std::fs::read_to_string("/proc/version").unwrap_or_default();
    platform_of(&v)
}

/// From the `/proc/version` text: WSL 1 kernels say `-Microsoft`, WSL 2 kernels `-microsoft-standard-WSL2`.
pub fn platform_of(version: &str) -> Platform {
    if version.contains("-Microsoft") {
        Platform::Wsl1
    } else if version.to_lowercase().contains("microsoft") {
        Platform::Wsl2
    } else {
        Platform::Linux
    }
}

/// MemAvailable of `/proc/meminfo` in MB.
fn available_mb() -> Option<u64> {
    let t = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = t.lines().find(|l| l.starts_with("MemAvailable:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

#[cfg(target_os = "linux")]
fn free_disk_mb(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: c is a valid C string, s a valid out pointer
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    Some((s.f_bavail as u64).saturating_mul(s.f_frsize as u64) / (1024 * 1024))
}

#[cfg(not(target_os = "linux"))]
fn free_disk_mb(_dir: &Path) -> Option<u64> {
    None
}

/// The IPv4 address this computer reaches other networks from (no packet is sent: a UDP `connect` only picks the
/// route). Under WSL 2 in its default network mode this is the Linux side's address, which Windows reaches.
pub fn local_address() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(v) if !v.is_unspecified() => Some(v),
        _ => None,
    }
}

/// Can srp bind its three addresses? Err = the first one in use, in words.
pub fn check_ports(c: &Config) -> Result<String, String> {
    let busy = |what: &str, a: SocketAddr, e: std::io::Error| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            format!("{what} {a} is in use: is srp already running (scripts/srp-host.sh status)? Stop it, or choose another port in [network]")
        } else {
            format!("{what} {a}: {e}")
        }
    };
    TcpListener::bind(c.login_addr).map_err(|e| busy("login tcp", c.login_addr, e))?;
    UdpSocket::bind(c.lobby_bind).map_err(|e| busy("lobby udp", c.lobby_bind, e))?;
    UdpSocket::bind(c.online.bind).map_err(|e| busy("matches udp", c.online.bind, e))?;
    TcpListener::bind(c.update_addr).map_err(|e| busy("update check tcp", c.update_addr, e))?;
    Ok(format!("login tcp {}, lobby udp {}, matches udp {}, update check tcp {}", c.login_addr, c.lobby_bind, c.online.bind, c.update_addr))
}

/// A TOML string literal.
pub fn toml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Set `key = value` lines of the `[section]` table of a config text, keeping every comment. A key that is there is
/// replaced; a missing one goes after its commented example (`#: key = ...`) if there is one, else after the section's
/// last key line.
pub fn set_keys(text: &str, section: &str, keys: &[(&str, String)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = format!("[{section}]");
    let start = match lines.iter().position(|l| l.trim() == header) {
        Some(i) => i,
        None => {
            lines.push(String::new());
            lines.push(header);
            lines.len() - 1
        }
    };
    for (k, v) in keys {
        let end = lines.iter().enumerate().skip(start + 1).find(|(_, l)| l.trim_start().starts_with('[')).map_or(lines.len(), |(i, _)| i);
        let is_key = |l: &str, prefix: &str| l.trim_start().strip_prefix(prefix).is_some_and(|r| r.strip_prefix(*k).is_some_and(|r| r.trim_start().starts_with('=')));
        let new = format!("{k} = {v}");
        if let Some(i) = (start + 1..end).find(|&i| is_key(&lines[i], "")) {
            lines[i] = new;
        } else if let Some(i) = (start + 1..end).find(|&i| is_key(&lines[i], "#: ")) {
            lines.insert(i + 1, new);
        } else {
            let last = (start + 1..end).rev().find(|&i| {
                let t = lines[i].trim();
                !t.is_empty() && !t.starts_with('#')
            });
            lines.insert(last.unwrap_or(start) + 1, new);
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The network keys for players at `address` (IPv4): every service binds all interfaces, the clients are told `address`.
pub fn network_keys(address: Ipv4Addr, c: &Config) -> Vec<(&'static str, String)> {
    vec![
        ("login", toml_str(&format!("0.0.0.0:{}", c.login_addr.port()))),
        ("lobby", toml_str(&format!("0.0.0.0:{}", c.lobby_bind.port()))),
        ("lobby_public", toml_str(&format!("{address}:{}", c.lobby_bind.port()))),
        ("match", toml_str(&format!("0.0.0.0:{}", c.online.bind.port()))),
        ("match_public", toml_str(&address.to_string())),
    ]
}

/// The host players put in `-server=` ([`Config::player_host`]; `<server address>` when every address is 0.0.0.0).
pub fn player_host(c: &Config) -> String {
    c.player_host().unwrap_or_else(|| "<server address>".into())
}

/// The command line players start the game with. No login key on it: the 0.69d game has no argument for one (it stops
/// at an unknown argument); the login server sends its public key in `0D` at every sign-in.
pub fn launch_line(c: &Config) -> String {
    format!("survarium.exe -product_name=Survarium-Steam -server={}:{} -localization=english", player_host(c), c.login_addr.port())
}

/// What players without Steam run once (`scripts/player/skhid-setup.cmd`): it fetches the start data from the update port
/// (`update::client_txt`) and writes the start file `Skhid.cmd` with [`launch_line`] on their desktop.
pub fn skhid_setup_command(c: &Config) -> String {
    let mut cmd = format!("skhid-setup.cmd -Server {}", player_host(c));
    if c.update_addr.port() != crate::update::DEFAULT_PORT {
        cmd.push_str(&format!(" -UpdatePort {}", c.update_addr.port()));
    }
    cmd
}

/// The login key: loaded, or generated (and written) when the file does not exist. The bool = generated now.
pub fn ensure_key(path: &Path) -> Result<(LoginKey, bool), String> {
    if path.exists() {
        return LoginKey::from_file(path).map(|k| (k, false)).map_err(|e| format!("login key {}: {e}", path.display()));
    }
    LoginKey::write_new(path).map(|k| (k, true)).map_err(|e| format!("login key {}: {e}", path.display()))
}

/// What `srp setup` was asked.
#[derive(Debug, Default)]
pub struct Options {
    /// The config file (`--config`, `SRP_CONFIG`, else `./srp.toml`).
    pub config: PathBuf,
    /// `--game`: the game folder (or the exe, or the Steam `Survarium` folder).
    pub game: Option<PathBuf>,
    /// `--address`: the IPv4 address players connect to.
    pub address: Option<Ipv4Addr>,
}

struct Report {
    problems: usize,
}

impl Report {
    fn line(&self, tag: &str, what: &str) {
        println!("  {tag:<8} {what}");
    }
    fn ok(&self, what: &str) {
        self.line("ok", what);
    }
    fn warn(&self, what: &str, fix: &str) {
        self.line("WARNING", what);
        if !fix.is_empty() {
            println!("           -> {fix}");
        }
    }
    fn problem(&mut self, what: &str, fix: &str) {
        self.problems += 1;
        self.line("PROBLEM", what);
        if !fix.is_empty() {
            println!("           -> {fix}");
        }
    }
}

/// Build the game tables from the install into `out` (`srp setup`, `srp extract`): every table of
/// [`game_data::DATA_FILES`], read from the game's resource volumes and survarium.exe (nothing is written into the game
/// folder). Returns (files, bytes, seconds).
pub fn extract_tables(g: &Game, out: &Path) -> Result<(usize, u64, f32), String> {
    let t0 = std::time::Instant::now();
    let src = data_extract::Source::open(&g.root, &g.exe).map_err(|e| e.to_string())?;
    let tables = data_extract::extract(&src).map_err(|e| e.to_string())?;
    data_extract::write(out, &tables).map_err(|e| e.to_string())?;
    let bytes = tables.iter().map(|t| t.bytes.len() as u64).sum();
    // the server loads exactly these files: check they read back
    game_data::GameData::from_dir(out).map_err(|e| format!("the tables written to {} do not load: {e}", out.display()))?;
    Ok((tables.len(), bytes, t0.elapsed().as_secs_f32()))
}

/// `srp setup`. Returns the exit code: 0 = ready to start, 2 = something needs fixing (said in the output).
pub fn run(o: &Options) -> i32 {
    let mut r = Report { problems: 0 };
    println!("srp setup: checking this computer for a Skhid server (Survarium 0.69d)\n");

    match platform() {
        Platform::Linux => r.ok("system: Linux"),
        Platform::Wsl2 => r.ok("system: Linux under WSL 2 (Windows)"),
        Platform::Wsl1 => r.problem("system: WSL 1: the world host needs WSL 2", "in Windows PowerShell: wsl --set-version <distribution> 2 (docs/guide-server.md)"),
        Platform::Other => {
            r.problem("system: the server runs on Linux or under WSL 2 on Windows", "see docs/guide-server.md");
        }
    }

    // the config: the existing file, or the example as the base of a new one
    let cfg_path = std::path::absolute(&o.config).unwrap_or_else(|_| o.config.clone());
    let existing = std::fs::read_to_string(&cfg_path).ok();
    let base = cfg_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let old = match &existing {
        Some(t) => match settings::Settings::parse(t).and_then(|s| Config::from_settings(s, &base)) {
            Ok(c) => Some(c),
            Err(e) => {
                r.problem(&format!("config {}: {e}", cfg_path.display()), "fix the line it names (every key is explained in config/srp.example.toml)");
                println!("\n{} problem(s): fix them and run srp setup again.", r.problems);
                return 2;
            }
        },
        None => None,
    };

    // the game
    let game = match (&o.game, old.as_ref().and_then(|c| c.world_host.daemon.as_ref())) {
        (Some(g), _) => game_at(g).ok_or_else(|| format!("no survarium.exe in {} (looked for binaries/x64/survarium.exe and game/binaries/x64/survarium.exe)", g.display())),
        (None, Some(d)) => game_at(&d.exe).ok_or_else(|| format!("world_host.exe {} does not exist", d.exe.display())),
        (None, None) => search_game().ok_or_else(|| "no Survarium install found in the usual Steam folders".to_string()),
    };
    let game = match game {
        Ok(g) => {
            r.ok(&format!("game folder: {}", g.root.display()));
            match check_exe(&g.exe) {
                Ok(()) => r.ok(&format!("survarium.exe: the 0.69d build ({} bytes, SHA-256 {}...{})", grouped(EXE_SIZE), &EXE_SHA256[..8], &EXE_SHA256[59..])),
                Err(e) => r.problem(&e, ""),
            }
            match check_resources(&g.root) {
                Ok(n) => r.ok(&format!("game resources: {n} resource files readable")),
                Err(e) => r.problem(&e, "verify the game files (Steam: Properties -> Installed files -> Verify integrity)"),
            }
            Some(g)
        }
        Err(e) => {
            r.problem(&e, "run srp setup --game <game folder> (the folder that holds binaries/x64/survarium.exe; under WSL a Windows drive C: is /mnt/c)");
            None
        }
    };

    // the world host program and its pre-warm lists
    let me_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let wh_bin = old.as_ref().and_then(|c| c.world_host.daemon.as_ref()).and_then(|d| d.bin.clone()).unwrap_or_else(|| me_dir.join("world-host"));
    if wh_bin.is_file() {
        r.ok(&format!("world host program: {}", wh_bin.display()));
    } else {
        r.problem(&format!("no world host program at {}", wh_bin.display()), "build it next to srp: cargo build --release -p srp -p world-host (docs/guide-server.md)");
    }

    // write or update the config
    let mut text = existing.clone().unwrap_or_else(|| format!("# Written by `srp setup`. Every key is explained below; change what you need and restart srp.\n{EXAMPLE}"));
    let template = Config::from_settings(settings::Settings::default(), &base).ok();
    let reference = old.clone().or(template);
    let mut changed = Vec::new();
    if let (Some(g), Some(c)) = (&game, &reference) {
        if c.world_host.daemon.as_ref().is_none_or(|d| d.exe != g.exe) {
            text = set_keys(&text, "world_host", &[("exe", toml_str(&g.exe.display().to_string()))]);
            changed.push("world_host.exe");
        }
    }
    let address = o.address.or_else(|| if existing.is_none() { local_address() } else { None });
    if let (Some(a), Some(c)) = (address, &reference) {
        text = set_keys(&text, "network", &network_keys(a, c));
        changed.push("network");
    }
    if existing.as_deref() != Some(text.as_str()) {
        let parsed = settings::Settings::parse(&text).and_then(|s| Config::from_settings(s, &base));
        if let Err(e) = parsed {
            r.problem(&format!("the config srp setup would write is not valid: {e}"), "please report this");
        } else if let Err(e) = std::fs::create_dir_all(&base).and_then(|_| std::fs::write(&cfg_path, &text)) {
            r.problem(&format!("cannot write {}: {e}", cfg_path.display()), "");
        } else if existing.is_none() {
            r.line("wrote", &format!("config {} (every setting is explained inside)", cfg_path.display()));
        } else {
            r.line("updated", &format!("config {}: {}", cfg_path.display(), changed.join(", ")));
        }
    } else {
        r.ok(&format!("config: {}", cfg_path.display()));
    }
    let c = match Config::load(Some(cfg_path.as_path())) {
        Ok((c, _)) => c,
        Err(e) => {
            r.problem(&e, "");
            println!("\n{} problem(s): fix them and run srp setup again.", r.problems);
            return 2;
        }
    };

    // the game tables, built from the install (the server ships none)
    let exe_ok = game.as_ref().is_some_and(|g| check_exe(&g.exe).is_ok());
    match (&game, &c.data_dir) {
        (Some(g), Some(dd)) if exe_ok => match extract_tables(g, dd) {
            Ok((n, bytes, secs)) => r.ok(&format!("game tables: {n} files built from the game install into {} ({:.1} MB, {secs:.1} s)", dd.display(), bytes as f64 / 1e6)),
            Err(e) => r.problem(&format!("game tables: {e}"), "verify the game files (Steam: Properties -> Installed files -> Verify integrity) and run srp setup again"),
        },
        (_, None) => r.problem("files.data_dir is empty: there is no folder for the game tables", "set files.data_dir = \"data\" in the config"),
        _ => r.problem("game tables: not built (the game folder or survarium.exe is not right, see above)", "fix the game folder and run srp setup again"),
    }
    if let Some(p) = c.world_host.daemon.as_ref().and_then(|d| crate::world_host::prewarm_dir(&c, d)) {
        match std::fs::create_dir_all(&p) {
            Ok(()) => r.ok(&format!("pre-warm lists: {} (every match adds what it loaded to its map's list)", p.display())),
            Err(e) => r.warn(&format!("pre-warm folder {}: {e}", p.display()), "set world_host.prewarm_dir to a writable folder"),
        }
    }

    // ports, memory, disk
    match check_ports(&c) {
        Ok(s) => r.ok(&format!("ports free: {s}")),
        Err(e) => r.problem(&e, ""),
    }
    let (zyg, per_map) = c.world_host.daemon.as_ref().map_or((8, 4), |d| (d.max_zygotes, d.max_matches));
    if let Some(mb) = available_mb() {
        let need = memory_need_mb(zyg, per_map);
        let txt = format!(
            "memory: {:.1} GB available{}; {}",
            mb as f64 / 1024.0,
            need.map_or(String::new(), |n| format!(
                " (the server uses up to about {:.1} GB with world_host.max_zygotes = {zyg} and max_matches = {per_map}, every map full)",
                n as f64 / 1024.0
            )),
            memory_fit_text(mb, per_map)
        );
        let one = BASE_MB + ZYGOTE_MB + MATCH_MB;
        match need {
            Some(_) if mb < one => r.problem(&format!("{txt}: too little for even one map with one match ({:.1} GB)", one as f64 / 1024.0), "close other programs, or give WSL more memory (docs/guide-server.md)"),
            Some(n) if mb < n => r.warn(
                &txt,
                &format!(
                    "set world_host.max_zygotes = {} (the maps loaded at once that fit in this memory with max_matches = {per_map}; docs/guide-server.md, \"Memory: how many maps and matches\") or add memory",
                    zygotes_that_fit(mb, per_map)
                ),
            ),
            _ => r.ok(&txt),
        }
    }
    let data_dir = c.db_path.parent().map(Path::to_path_buf).unwrap_or_else(|| base.clone());
    if let Some(mb) = free_disk_mb(&data_dir).or_else(|| free_disk_mb(&base)) {
        let txt = format!("disk: {:.1} GB free for the database and logs", mb as f64 / 1024.0);
        if mb < DISK_WARN_MB { r.warn(&txt, "free some disk space: the logs grow with every match") } else { r.ok(&txt) }
    }

    // database and login key
    let new_db = !c.db_path.exists();
    match accounts::Store::open(&c.db_path) {
        Ok(s) => {
            let n = s.account_count().unwrap_or(0);
            r.line(if new_db { "created" } else { "ok" }, &format!("database {} ({n} account(s))", c.db_path.display()));
        }
        Err(e) => r.problem(&format!("database {}: {e}", c.db_path.display()), "check that the folder exists and is writable"),
    }
    let key_ok = match ensure_key(&c.key_path) {
        Ok((_, created)) => {
            if created {
                r.line("created", &format!("login key {} (keep it secret: it decrypts the players' passwords)", c.key_path.display()));
            } else {
                r.ok(&format!("login key {}", c.key_path.display()));
            }
            true
        }
        Err(e) => {
            r.problem(&e, "move the broken file away; srp setup then makes a new key (players need nothing new)");
            false
        }
    };

    println!();
    if r.problems == 0 {
        println!("Setup complete. Check the game's world code once (about a minute), then start the server:");
        println!("    scripts/srp-host.sh selftest {}", cfg_path.display());
        println!("    scripts/srp-host.sh run {}      (foreground; keep the window open, Ctrl+C stops it)", cfg_path.display());
        if platform() != Platform::Wsl2 {
            println!("    scripts/srp-host.sh start {}    (or in the background; stop with: scripts/srp-host.sh stop)", cfg_path.display());
        }
    } else {
        println!("{} problem(s): fix them and run srp setup again.", r.problems);
    }
    if key_ok {
        let host = player_host(&c);
        println!("\nPlayers set up their game once with one of the scripts (docs/guide-player.md):");
        println!("    Survarium in their Steam library: scripts/steam-play/steam-play-setup.cmd -Server {host}   (then Steam's Play button)");
        println!("    without Steam:                    scripts/player/{}   (writes the start file Skhid.cmd)", skhid_setup_command(&c));
        println!("\nBy hand, the start file Skhid.cmd starts the game with this line (in the game's game\\binaries\\x64 folder):\n");
        println!("    {}\n", launch_line(&c));
        let local = host.parse::<Ipv4Addr>().is_ok_and(|a| a.is_loopback());
        if local {
            println!("The address {host} works only on this computer. For other players run: srp setup --address <IP players reach>");
        } else {
            println!("Players reach this server at {host}. Players from the internet need your public IP address and the ports forwarded");
            println!(
                "(TCP {} and {}, UDP {} and {}): docs/guide-server.md, \"Letting players in\".",
                c.login_addr.port(),
                c.update_addr.port(),
                c.lobby_bind.port(),
                c.online.bind.port()
            );
            if platform() == Platform::Wsl2 {
                println!("Under WSL 2 this address can change when Windows restarts: run srp setup --address <IP> again then, or use");
                println!("WSL's mirrored network mode (docs/guide-server.md).");
            }
        }
        println!(
            "\nBoth scripts need TCP {} reachable (the Steam launcher's update check and the start data); srp check prints the exact lines.",
            c.update_addr.port()
        );
        if c.auto_register {
            println!("\nAccounts: the first sign-in with a new name creates the account (accounts.auto_register = true).");
        } else {
            println!("\nAccounts: registration is closed (accounts.auto_register = false). Create accounts with:");
            println!("    srp --config {} account add <name> <password>", cfg_path.display());
        }
    }
    if r.problems == 0 { 0 } else { 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_memory_suggestion_follows_the_guides_worst_case_table() {
        // 0.35 GB + max_zygotes x (0.35 GB + max_matches x 0.165 GB)
        assert_eq!(memory_need_mb(0, 4), None, "no cap");
        assert_eq!(memory_need_mb(1, 4), Some(350 + 350 + 4 * 165));
        assert_eq!(memory_need_mb(8, 4), Some(350 + 8 * 1010));
        // the worst-case column of the guide's table (machine memory minus about 0.5 GB for the system)
        assert_eq!(zygotes_that_fit(2048 - 512, 4), 1);
        assert_eq!(zygotes_that_fit(4096 - 512, 4), 3);
        assert_eq!(zygotes_that_fit(8192 - 512, 4), 7);
        assert_eq!(zygotes_that_fit(16384 - 512, 4), 15);
        assert_eq!(zygotes_that_fit(100, 4), 1, "never below one");
        assert_eq!(zygotes_that_fit(4096 - 512, 2), 4, "fewer matches per map: more maps (3234 / 680)");
    }

    #[test]
    fn setup_prints_the_worst_case_fit_and_the_usual_pvp_fit() {
        // the guide's "Suggested settings by memory", usual PvP maps: 0.35 GB + n x (0.085 GB + max_matches x 0.13 GB)
        assert_eq!(typical_pvp_maps_that_fit(2048 - 512, 4), 1);
        assert_eq!(typical_pvp_maps_that_fit(4096 - 512, 4), 5);
        assert_eq!(typical_pvp_maps_that_fit(8192 - 512, 4), 12);
        assert_eq!(typical_pvp_maps_that_fit(16384 - 512, 4), 25);
        assert_eq!(typical_pvp_maps_that_fit(100, 4), 1, "never below one");
        assert_eq!(typical_pvp_maps_that_fit(4096 - 512, 2), 9, "3234 / 345");
        assert_eq!(
            memory_fit_text(8192 - 512, 4),
            "this memory fits 7 map(s) in the worst case (each map its largest kind, every match slot used) or about 12 usual PvP maps, with max_matches = 4"
        );
    }

    #[test]
    fn sha256_matches_the_standard_vectors() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(hex(&sha256(&million)), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn another_exe_is_refused_in_words() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("survarium.exe");
        std::fs::write(&exe, b"MZ not the game").unwrap();
        let e = check_exe(&exe).unwrap_err();
        assert!(e.contains("not the Survarium 0.69d build") && e.contains(EXE_SHA256) && e.contains("13,386,392"), "{e}");
        assert!(check_exe(&dir.path().join("missing.exe")).unwrap_err().contains("cannot read"));
    }

    #[test]
    fn the_game_is_found_from_any_of_its_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Survarium/game");
        std::fs::create_dir_all(root.join("binaries/x64")).unwrap();
        std::fs::create_dir_all(root.join("resources")).unwrap();
        std::fs::write(root.join("binaries/x64/survarium.exe"), b"x").unwrap();
        let want = Game { root: root.clone(), exe: root.join("binaries/x64/survarium.exe") };
        for p in [dir.path().join("Survarium"), root.clone(), root.join("binaries/x64"), root.join("binaries/x64/survarium.exe")] {
            assert_eq!(game_at(&p).as_ref(), Some(&want), "{}", p.display());
        }
        assert_eq!(game_at(dir.path()), None);
        assert!(check_resources(&root).unwrap_err().contains("no resources.NN.db"));
        std::fs::write(root.join("resources.00.db"), b"db").unwrap();
        std::fs::write(root.join("resources.75.db.orig"), b"old").unwrap();
        assert_eq!(check_resources(&root), Ok(1));
    }

    #[test]
    fn steam_library_folders_become_linux_paths() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n\t\"2\"\n\t{\n\t\t\"path\"\t\t\"/home/me/games\"\n\t}\n}\n";
        assert_eq!(
            steam_libraries(vdf),
            vec![PathBuf::from("/mnt/c/Program Files (x86)/Steam"), PathBuf::from("/mnt/d/SteamLibrary"), PathBuf::from("/home/me/games")]
        );
    }

    #[test]
    fn wsl_versions_are_told_apart() {
        assert_eq!(platform_of("Linux version 5.15.153.1-microsoft-standard-WSL2 (root@host) #1 SMP"), Platform::Wsl2);
        assert_eq!(platform_of("Linux version 4.4.0-19041-Microsoft (builder@example.com)"), Platform::Wsl1);
        assert_eq!(platform_of("Linux version 6.8.0-45-generic (buildd@lcy02) #45-Ubuntu SMP"), Platform::Linux);
    }

    /// The config srp setup writes: the example with the game, the pre-warm folder and the addresses set, still valid,
    /// every comment kept.
    #[test]
    fn setup_fills_the_example_config() {
        let base = Path::new("/srv/srp");
        let def = Config::from_settings(settings::Settings::default(), base).unwrap();
        let mut t = EXAMPLE.to_string();
        t = set_keys(&t, "world_host", &[("exe", toml_str("/opt/survarium/game/binaries/x64/survarium.exe"))]);
        t = set_keys(&t, "world_host", &[("prewarm_dir", toml_str("/srv/skhid/data/prewarm"))]);
        t = set_keys(&t, "network", &network_keys(Ipv4Addr::new(203, 0, 113, 20), &def));
        let c = Config::from_settings(settings::Settings::parse(&t).unwrap(), base).unwrap();
        let d = c.world_host.daemon.as_ref().unwrap();
        assert_eq!(d.exe, PathBuf::from("/opt/survarium/game/binaries/x64/survarium.exe"));
        assert_eq!(d.exe_win, "G:\\binaries\\x64\\survarium.exe");
        assert_eq!(d.prewarm_dir.as_deref(), Some(Path::new("/srv/skhid/data/prewarm")));
        assert_eq!(c.login_addr.to_string(), "0.0.0.0:65000");
        assert_eq!(c.lobby_public.to_string(), "203.0.113.20:1235");
        assert_eq!(c.online.public_host, "203.0.113.20");
        assert_eq!(c.online.bind.to_string(), "0.0.0.0:1236");
        // the exe line sits right under its commented example, comments stay
        let lines: Vec<&str> = t.lines().collect();
        let i = lines.iter().position(|l| l.starts_with("exe = ")).unwrap();
        assert!(lines[i - 1].starts_with("#: exe = "), "{}", lines[i - 1]);
        assert_eq!(t.lines().filter(|l| l.starts_with('#')).count(), EXAMPLE.lines().filter(|l| l.starts_with('#')).count());
        // setting again replaces, never duplicates
        let again = set_keys(&t, "network", &network_keys(Ipv4Addr::new(198, 51, 100, 5), &def));
        assert_eq!(again.lines().filter(|l| l.starts_with("match_public = ")).count(), 1);
        assert!(again.contains("match_public = \"198.51.100.5\""));
        let p = Config::from_settings(settings::Settings::parse(&again).unwrap(), base).unwrap();
        assert_eq!(p.online.public_host, "198.51.100.5");
    }

    #[test]
    fn the_launch_line_has_the_server_and_no_key() {
        let mut c = Config::default();
        c.online.public_host = "203.0.113.7".into();
        c.login_addr = "0.0.0.0:65000".parse().unwrap();
        let l = launch_line(&c);
        assert_eq!(l, "survarium.exe -product_name=Survarium-Steam -server=203.0.113.7:65000 -localization=english");
        assert!(!l.contains("login_public_key"), "the 0.69d game stops at an unregistered argument");
        assert_eq!(skhid_setup_command(&c), "skhid-setup.cmd -Server 203.0.113.7");
        c.update_addr = "0.0.0.0:7000".parse().unwrap();
        assert_eq!(skhid_setup_command(&c), "skhid-setup.cmd -Server 203.0.113.7 -UpdatePort 7000");
    }
}
