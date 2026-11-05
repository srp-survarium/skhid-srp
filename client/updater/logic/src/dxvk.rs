//! The opt-in Vulkan renderer (`srp-026g/docs/notes/dxvk-option.md`): the
//! game imports `d3d11.dll` and `dxgi.dll`, and Windows takes them from the
//! game's own folder first. The Skhid ZIP ships DXVK's two in
//! `binaries\x86\dxvk\`; at every update the updater puts them beside the game
//! when the player's Settings ask for it (user.cfg's `srp_dxvk on`) and a
//! Vulkan device can run DXVK, and takes them away otherwise. Only a file
//! with one of [`OURS`]'s hashes is ever replaced or removed there: a
//! `d3d11.dll` of the player's own stays.
//!
//! With DXVK put beside the game the updater leaves [`MARKER`] there; the game
//! deletes it once its lobby shows (exe patch 83). A marker still there at the
//! next run, with ours beside the game, is a start with DXVK that never got
//! there: the setting is turned off in user.cfg and plain DirectX is back
//! ([`step`]).

use alloc::vec::Vec;

use crate::replay::user_cfg_value;
use crate::sha256::from_hex;

/// The folder beside the updater that holds DXVK's DLLs (the ZIP's
/// `game/binaries/x86/dxvk/`).
pub const FOLDER: &str = "dxvk";
/// DXVK's DLLs the game loads, in the order of [`decide`]'s arrays.
pub const DLLS: [&str; 2] = ["d3d11.dll", "dxgi.dll"];
/// The user.cfg variable of the Settings' checkbox (exe patch 83).
pub const SETTING: &[u8] = b"srp_dxvk";
/// Beside the updater (and the game, whose working folder it is): DXVK was
/// put in place for this start, which has not shown the lobby yet.
pub const MARKER: &str = "dxvk.pending";

/// The SHA-256 of every DXVK release a Skhid ZIP has shipped, its two DLLs
/// in the order of [`DLLS`]: DXVK 3.1.1's x32 `d3d11.dll` and `dxgi.dll`
/// (`dxvk-3.1.1.tar.gz`, sha256 `40565b4a…`, `make_client_026g.py`'s
/// `DXVK_DLLS`). A later DXVK adds its row and keeps these, so that the copies
/// an earlier ZIP put beside the game are known.
pub const OURS: [[&str; 2]; 1] = [[
    "f44543cbe5c40202915a02ed6e9ac8140c3524e846ae67b01f6faa79485d1fd3",
    "abd1449dc28d536a4289c256ae7f4b6bbd15fa552792b7170c2ddece473c8bda",
]];

/// The Vulkan version DXVK 3 needs of a device: 1.4.
pub const NEEDED: Version = Version::new(1, 4, 0);

/// Whether user.cfg asks for DXVK: its `srp_dxvk on` (the game writes `on`
/// or `off`; anything else, or no line, is off).
pub fn asked(user_cfg: &[u8]) -> bool {
    user_cfg_value(user_cfg, SETTING) == Some(b"on")
}

/// The row of [`OURS`] whose DLL `dll` (an index of [`DLLS`]) has `hash`.
fn release(dll: usize, hash: &[u8; 32]) -> Option<usize> {
    OURS.iter()
        .position(|row| from_hex(row[dll].as_bytes()).as_ref() == Some(hash))
}

/// Whether `hash` is one of [`OURS`]' for the DLL `dll`.
pub fn is_ours(dll: usize, hash: &[u8; 32]) -> bool {
    release(dll, hash).is_some()
}

/// user.cfg with the setting off: the value of its `srp_dxvk` line (the one
/// [`asked`] reads) replaced by `off`, every other byte as it was. None when
/// it has no such line.
pub fn turned_off(user_cfg: &[u8]) -> Option<Vec<u8>> {
    let value = user_cfg_value(user_cfg, SETTING)?;
    let at = value.as_ptr() as usize - user_cfg.as_ptr() as usize;
    let mut text = user_cfg[..at].to_vec();
    text.extend_from_slice(b"off");
    text.extend_from_slice(&user_cfg[at + value.len()..]);
    Some(text)
}

/// A Vulkan version as Vulkan packs it (`VK_MAKE_API_VERSION`, variant 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(u32);

impl Version {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Version {
        Version((major << 22) | (minor << 12) | patch)
    }

    /// A device's `apiVersion`; None for another variant than 0 (not a
    /// Vulkan version).
    pub fn of_packed(packed: u32) -> Option<Version> {
        (packed >> 29 == 0).then_some(Version(packed))
    }

    pub fn packed(self) -> u32 {
        self.0
    }

    pub fn major(self) -> u32 {
        self.0 >> 22
    }

    pub fn minor(self) -> u32 {
        (self.0 >> 12) & 0x3FF
    }

    pub fn patch(self) -> u32 {
        self.0 & 0xFFF
    }
}

impl core::fmt::Display for Version {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major(), self.minor(), self.patch())
    }
}

/// What the updater's `vulkan` run found (it runs apart, so that a driver
/// that crashes or hangs in it takes nothing else down), told by its exit
/// code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// The best Vulkan version of a device that is not the CPU.
    Device(Version),
    /// No `vulkan-1.dll`, or not its functions.
    NoLoader,
    /// `vkCreateInstance` failed.
    NoInstance,
    /// No device but the CPU.
    NoDevice,
    /// The run ended otherwise (a crash, a hang cut short): its exit code.
    Failed(u32),
}

const NO_LOADER: u32 = 11;
const NO_INSTANCE: u32 = 12;
const NO_DEVICE: u32 = 13;

impl Probe {
    pub fn exit_code(self) -> u32 {
        match self {
            Probe::Device(version) => version.packed(),
            Probe::NoLoader => NO_LOADER,
            Probe::NoInstance => NO_INSTANCE,
            Probe::NoDevice => NO_DEVICE,
            Probe::Failed(code) => code,
        }
    }

    pub fn of_exit_code(code: u32) -> Probe {
        match code {
            NO_LOADER => Probe::NoLoader,
            NO_INSTANCE => Probe::NoInstance,
            NO_DEVICE => Probe::NoDevice,
            _ => match Version::of_packed(code) {
                Some(version) if version.major() >= 1 => Probe::Device(version),
                _ => Probe::Failed(code),
            },
        }
    }

    /// Whether DXVK can run on what was found.
    pub fn suffices(self) -> bool {
        matches!(self, Probe::Device(version) if version >= NEEDED)
    }
}

/// For the log.
impl core::fmt::Display for Probe {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Probe::Device(version) => write!(f, "the best Vulkan device is {version}"),
            Probe::NoLoader => f.write_str("no Vulkan (vulkan-1.dll)"),
            Probe::NoInstance => f.write_str("Vulkan did not start (vkCreateInstance)"),
            Probe::NoDevice => f.write_str("no Vulkan device but the CPU"),
            Probe::Failed(code) => write!(f, "the Vulkan check ended with code {code:#x}"),
        }
    }
}

/// A file at one of the two places (beside the game, or in [`FOLDER`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    /// Not there.
    Missing,
    /// One of [`OURS`] for its DLL, with its hash.
    Ours([u8; 32]),
    /// Anything else: another file, or one that cannot be read.
    Other,
}

impl File {
    /// The file of DLL `dll` (an index of [`DLLS`]) with `hash`.
    pub fn of_hash(dll: usize, hash: [u8; 32]) -> File {
        if is_ours(dll, &hash) {
            File::Ours(hash)
        } else {
            File::Other
        }
    }
}

/// What the run finds, before anything is changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    /// user.cfg says `srp_dxvk on`.
    pub asked: bool,
    /// [`MARKER`] is there.
    pub marker: bool,
    /// Beside the game, and in [`FOLDER`], in the order of [`DLLS`].
    pub beside: [File; 2],
    pub source: [File; 2],
}

/// Where DXVK stands at the start of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The setting is off.
    Off,
    /// On; none of ours beside the game: not started with DXVK yet.
    NotStarted,
    /// On; ours beside the game and no marker: the last start showed the
    /// lobby with them.
    Confirmed,
    /// On; ours beside the game and the marker still there: the last start
    /// with them never showed the lobby.
    Failed,
}

impl State {
    pub fn of(found: &Found) -> State {
        let in_use = found
            .beside
            .iter()
            .any(|file| matches!(file, File::Ours(_)));
        match (found.asked, in_use, found.marker) {
            (false, _, _) => State::Off,
            (true, false, _) => State::NotStarted,
            (true, true, false) => State::Confirmed,
            (true, true, true) => State::Failed,
        }
    }
}

/// Why DXVK is not used this run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unused {
    /// The setting is off.
    Off,
    /// No Vulkan device can run it.
    Vulkan(Probe),
    /// [`FOLDER`] lacks this DLL of a release of ours (gone, unreadable,
    /// other bytes), or its two are not of one release.
    NoSource(usize),
    /// This DLL beside the game is the player's own: it stays.
    Foreign(usize),
}

/// What the run does. Every plan takes ours away but [`Plan::Use`], and
/// deletes the marker but [`Plan::Use`], which leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// DXVK for this start: these DLLs (indices of [`DLLS`]) copied from
    /// [`FOLDER`] beside the game (the others are there), the marker left.
    Use { copy: Vec<usize> },
    /// Plain DirectX: these of ours removed from beside the game.
    Unused { remove: Vec<usize>, why: Unused },
    /// The last start with DXVK never showed the lobby: user.cfg's setting
    /// turned off ([`turned_off`]), these of ours removed.
    TurnOff { remove: Vec<usize> },
}

/// The run's plan from what it found. `vulkan` is asked only when DXVK may
/// be used (the setting on, the last start not failed).
pub fn step(found: &Found, vulkan: impl FnOnce() -> Probe) -> Plan {
    let which =
        |pick: &dyn Fn(usize) -> bool| (0..DLLS.len()).filter(|&i| pick(i)).collect::<Vec<usize>>();
    let ours = which(&|i| matches!(found.beside[i], File::Ours(_)));
    let unused = |why| Plan::Unused {
        remove: ours.clone(),
        why,
    };
    match State::of(found) {
        State::Off => unused(Unused::Off),
        State::Failed => Plan::TurnOff {
            remove: ours.clone(),
        },
        State::NotStarted | State::Confirmed => {
            let releases = [0, 1].map(|i| match found.source[i] {
                File::Ours(hash) => release(i, &hash),
                File::Missing | File::Other => None,
            });
            if let Some(i) = (0..DLLS.len()).find(|&i| releases[i].is_none()) {
                return unused(Unused::NoSource(i));
            }
            if releases[0] != releases[1] {
                return unused(Unused::NoSource(1));
            }
            if let Some(i) = (0..DLLS.len()).find(|&i| found.beside[i] == File::Other) {
                return unused(Unused::Foreign(i));
            }
            let probe = vulkan();
            if !probe.suffices() {
                return unused(Unused::Vulkan(probe));
            }
            Plan::Use {
                copy: which(&|i| found.beside[i] != found.source[i]),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn ours(i: usize) -> File {
        File::Ours(from_hex(OURS[0][i].as_bytes()).unwrap())
    }

    const BOTH: [File; 2] = [File::Missing; 2];

    fn found(asked: bool, marker: bool, beside: [File; 2]) -> Found {
        Found {
            asked,
            marker,
            beside,
            source: [ours(0), ours(1)],
        }
    }

    fn good() -> Probe {
        Probe::Device(Version::new(1, 4, 303))
    }

    fn unasked() -> Probe {
        panic!("Vulkan asked")
    }

    #[test]
    fn user_cfg_asks_with_on_only() {
        assert!(asked(
            b"localization russian\r\nsrp_dxvk on\r\nr_fullscreen on\r\n"
        ));
        assert!(asked(b"srp_dxvk\ton"));
        assert!(!asked(b"srp_dxvk off\r\n"));
        assert!(!asked(b"srp_dxvk 1\r\n"));
        assert!(!asked(b"srp_dxvk\r\n"));
        assert!(!asked(b"srp_dxvk_old on\r\n# srp_dxvk on\r\n"));
        assert!(!asked(b""));
    }

    #[test]
    fn ours_are_the_pinned_hashes_each_for_its_dll() {
        for (i, hash) in OURS[0].iter().enumerate() {
            let hash = from_hex(hash.as_bytes()).unwrap();
            assert!(is_ours(i, &hash));
            assert!(!is_ours(1 - i, &hash));
            assert_eq!(File::of_hash(i, hash), ours(i));
            assert_eq!(File::of_hash(1 - i, hash), File::Other);
        }
        assert_eq!(File::of_hash(0, [0; 32]), File::Other);
    }

    #[test]
    fn turning_off_rewrites_the_setting_alone() {
        let text = b"localization russian\r\nsrp_dxvk on\r\nr_fullscreen on\r\n";
        assert_eq!(
            turned_off(text).unwrap(),
            b"localization russian\r\nsrp_dxvk off\r\nr_fullscreen on\r\n"
        );
        assert_eq!(turned_off(b"srp_dxvk\ton").unwrap(), b"srp_dxvk\toff");
        assert!(!asked(&turned_off(text).unwrap()));
        assert_eq!(turned_off(b"localization russian\r\n"), None);
    }

    #[test]
    fn versions_unpack_and_compare() {
        let version = Version::new(1, 3, 280);
        assert_eq!(
            (version.major(), version.minor(), version.patch()),
            (1, 3, 280)
        );
        assert_eq!(alloc::format!("{version}"), "1.3.280");
        assert!(Version::new(1, 4, 0) > Version::new(1, 3, 4095));
        assert!(Version::new(2, 0, 0) > Version::new(1, 4, 9));
        assert_eq!(
            Version::of_packed(0x0040_3118),
            Some(Version::new(1, 3, 280))
        );
        // a variant other than 0 is no version
        assert_eq!(Version::of_packed(0xC000_0005), None);
    }

    #[test]
    fn the_probe_travels_in_the_exit_code() {
        for probe in [
            Probe::Device(Version::new(1, 4, 303)),
            Probe::Device(Version::new(1, 0, 0)),
            Probe::NoLoader,
            Probe::NoInstance,
            Probe::NoDevice,
        ] {
            assert_eq!(Probe::of_exit_code(probe.exit_code()), probe);
        }
        // a crash, the updater's own internal error, a run cut short
        for code in [0xC000_0005, 3, 1, 0, 1223] {
            assert_eq!(Probe::of_exit_code(code), Probe::Failed(code));
        }
    }

    #[test]
    fn dxvk_runs_on_vulkan_1_4_and_later() {
        assert!(Probe::Device(Version::new(1, 4, 0)).suffices());
        assert!(Probe::Device(Version::new(1, 4, 303)).suffices());
        assert!(!Probe::Device(Version::new(1, 3, 280)).suffices());
        assert_eq!(
            alloc::format!("{}", Probe::Device(Version::new(1, 3, 280))),
            "the best Vulkan device is 1.3.280"
        );
        for probe in [
            Probe::NoLoader,
            Probe::NoInstance,
            Probe::NoDevice,
            Probe::Failed(5),
        ] {
            assert!(!probe.suffices());
        }
    }

    #[test]
    fn off_removes_ours_only_and_asks_no_vulkan() {
        let plan = |beside| step(&found(false, false, beside), unasked);
        assert_eq!(
            plan([ours(0), ours(1)]),
            Plan::Unused {
                remove: vec![0, 1],
                why: Unused::Off
            }
        );
        assert_eq!(
            plan([File::Other, ours(1)]),
            Plan::Unused {
                remove: vec![1],
                why: Unused::Off
            }
        );
        assert_eq!(
            plan([File::Other; 2]),
            Plan::Unused {
                remove: vec![],
                why: Unused::Off
            }
        );
        assert_eq!(
            State::of(&found(false, true, [ours(0), ours(1)])),
            State::Off
        );
    }

    #[test]
    fn on_and_not_started_copies_both_and_leaves_the_marker() {
        let first = found(true, false, BOTH);
        assert_eq!(State::of(&first), State::NotStarted);
        assert_eq!(step(&first, good), Plan::Use { copy: vec![0, 1] });
        // a marker of a start before that took ours away: not a failure
        assert_eq!(
            step(&found(true, true, BOTH), good),
            Plan::Use { copy: vec![0, 1] }
        );
    }

    #[test]
    fn on_and_confirmed_keeps_them_and_leaves_the_marker_again() {
        let confirmed = found(true, false, [ours(0), ours(1)]);
        assert_eq!(State::of(&confirmed), State::Confirmed);
        assert_eq!(step(&confirmed, good), Plan::Use { copy: vec![] });
        // a release's copy of another hash is replaced
        assert_eq!(
            step(&found(true, false, [ours(1), ours(1)]), good),
            Plan::Use { copy: vec![0] }
        );
        // the driver no longer suffices: plain DirectX
        let old = Probe::Device(Version::new(1, 3, 280));
        assert_eq!(
            step(&confirmed, || old),
            Plan::Unused {
                remove: vec![0, 1],
                why: Unused::Vulkan(old)
            }
        );
    }

    #[test]
    fn on_and_pending_is_a_failed_start_turned_off() {
        let failed = found(true, true, [ours(0), ours(1)]);
        assert_eq!(State::of(&failed), State::Failed);
        assert_eq!(step(&failed, unasked), Plan::TurnOff { remove: vec![0, 1] });
        assert_eq!(
            step(&found(true, true, [File::Missing, ours(1)]), unasked),
            Plan::TurnOff { remove: vec![1] }
        );
    }

    #[test]
    fn on_without_a_vulkan_device_is_plain_directx() {
        for probe in [
            Probe::NoLoader,
            Probe::NoInstance,
            Probe::NoDevice,
            Probe::Failed(5),
        ] {
            assert_eq!(
                step(&found(true, false, BOTH), || probe),
                Plan::Unused {
                    remove: vec![],
                    why: Unused::Vulkan(probe)
                }
            );
        }
    }

    #[test]
    fn on_without_ours_in_the_folder_is_plain_directx() {
        // gone, unreadable, other bytes
        for (source, i) in [
            ([File::Missing, ours(1)], 0),
            ([ours(0), File::Other], 1),
            ([File::Other, File::Other], 0),
            // the two of one release only: d3d11's hash in dxgi's place
            (
                [
                    ours(0),
                    File::Ours(from_hex(OURS[0][0].as_bytes()).unwrap()),
                ],
                1,
            ),
        ] {
            let found = Found {
                source,
                ..found(true, false, [ours(0), ours(1)])
            };
            assert_eq!(
                step(&found, unasked),
                Plan::Unused {
                    remove: vec![0, 1],
                    why: Unused::NoSource(i)
                }
            );
        }
    }

    #[test]
    fn on_never_replaces_a_dll_of_the_players() {
        assert_eq!(
            step(&found(true, false, [File::Other, File::Missing]), unasked),
            Plan::Unused {
                remove: vec![],
                why: Unused::Foreign(0)
            }
        );
        assert_eq!(
            step(&found(true, false, [ours(0), File::Other]), unasked),
            Plan::Unused {
                remove: vec![0],
                why: Unused::Foreign(1)
            }
        );
    }
}
