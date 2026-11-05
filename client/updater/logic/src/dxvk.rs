//! The opt-in Vulkan renderer (`srp-026g/docs/notes/dxvk-option.md`): the
//! game imports `d3d11.dll` and `dxgi.dll`, and Windows takes them from the
//! game's own folder first. The Skhid ZIP ships DXVK's two in
//! `binaries\x86\dxvk\`; at every update the updater puts them beside the game
//! when the player's Settings ask for it (user.cfg's `srp_dxvk on`) and a
//! Vulkan device can run DXVK, and takes them away otherwise. Only a file
//! with one of [`OURS`]'s hashes is ever replaced or removed there: a
//! `d3d11.dll` of the player's own stays.

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

/// The SHA-256 of every DXVK DLL a Skhid ZIP has shipped: DXVK 3.1.1's x32
/// `d3d11.dll` and `dxgi.dll` (`dxvk-3.1.1.tar.gz`, sha256 `40565b4a…`,
/// `make_client_026g.py`'s `DXVK_DLLS`). A later DXVK adds its two and keeps
/// these, so that the copies an earlier ZIP put beside the game are known.
pub const OURS: [&str; 2] = [
    "f44543cbe5c40202915a02ed6e9ac8140c3524e846ae67b01f6faa79485d1fd3",
    "abd1449dc28d536a4289c256ae7f4b6bbd15fa552792b7170c2ddece473c8bda",
];

/// The Vulkan version DXVK 3 needs of a device: 1.4.
pub const NEEDED: Version = Version::new(1, 4, 0);

/// Whether user.cfg asks for DXVK: its `srp_dxvk on` (the game writes `on`
/// or `off`; anything else, or no line, is off).
pub fn asked(user_cfg: &[u8]) -> bool {
    user_cfg_value(user_cfg, SETTING) == Some(b"on")
}

/// Whether `hash` is one of [`OURS`].
pub fn is_ours(hash: &[u8; 32]) -> bool {
    OURS.iter()
        .any(|ours| from_hex(ours.as_bytes()).as_ref() == Some(hash))
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
    Missing,
    /// One of [`OURS`], with its hash.
    Ours([u8; 32]),
    /// Anything else: another file, or one that cannot be read.
    Other,
}

impl File {
    pub fn of_hash(hash: [u8; 32]) -> File {
        if is_ours(&hash) {
            File::Ours(hash)
        } else {
            File::Other
        }
    }
}

/// What the update does with the DLLs beside the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Copy these (indices of [`DLLS`]) from [`FOLDER`] beside the game; the
    /// others are there already.
    Copy(Vec<usize>),
    /// Remove these (ours) from beside the game.
    Remove(Vec<usize>),
    /// Nothing to change.
    Nothing,
    /// DXVK is wanted but not used: [`FOLDER`] lacks one of ours.
    NoSource(usize),
    /// DXVK is wanted but not used: this DLL beside the game is not ours
    /// (the player's own), and stays; ours stay as they are too.
    Foreign(usize),
}

/// The decision for DXVK on (`wanted`: asked and a device suffices) or
/// off, from what is beside the game and in [`FOLDER`]. Both DLLs or none:
/// DXVK's `d3d11.dll` does not run with Windows' `dxgi.dll`.
pub fn decide(wanted: bool, beside: [File; 2], source: [File; 2]) -> Decision {
    let which =
        |pick: &dyn Fn(usize) -> bool| (0..DLLS.len()).filter(|&i| pick(i)).collect::<Vec<usize>>();
    if !wanted {
        let ours = which(&|i| matches!(beside[i], File::Ours(_)));
        return if ours.is_empty() {
            Decision::Nothing
        } else {
            Decision::Remove(ours)
        };
    }
    if let Some(i) = (0..DLLS.len()).find(|&i| !matches!(source[i], File::Ours(_))) {
        return Decision::NoSource(i);
    }
    if let Some(i) = (0..DLLS.len()).find(|&i| beside[i] == File::Other) {
        return Decision::Foreign(i);
    }
    let copy = which(&|i| beside[i] != source[i]);
    if copy.is_empty() {
        Decision::Nothing
    } else {
        Decision::Copy(copy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn ours(i: usize) -> File {
        File::Ours(from_hex(OURS[i].as_bytes()).unwrap())
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
    fn ours_are_the_pinned_hashes() {
        for (i, hash) in OURS.iter().enumerate() {
            let hash = from_hex(hash.as_bytes()).unwrap();
            assert!(is_ours(&hash));
            assert_eq!(File::of_hash(hash), ours(i));
        }
        assert_eq!(File::of_hash([0; 32]), File::Other);
        assert_eq!(DLLS.len(), OURS.len());
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
    fn on_copies_what_is_not_in_place() {
        let source = [ours(0), ours(1)];
        assert_eq!(
            decide(true, [File::Missing, File::Missing], source),
            Decision::Copy(vec![0, 1])
        );
        assert_eq!(
            decide(true, [ours(0), File::Missing], source),
            Decision::Copy(vec![1])
        );
        assert_eq!(decide(true, source, source), Decision::Nothing);
        // an earlier ZIP's copy (here: the other DLL's hash) is replaced
        assert_eq!(
            decide(true, [ours(1), ours(1)], source),
            Decision::Copy(vec![0])
        );
    }

    #[test]
    fn on_never_replaces_a_dll_of_the_players() {
        let source = [ours(0), ours(1)];
        assert_eq!(
            decide(true, [File::Other, File::Missing], source),
            Decision::Foreign(0)
        );
        assert_eq!(
            decide(true, [ours(0), File::Other], source),
            Decision::Foreign(1)
        );
    }

    #[test]
    fn on_without_ours_in_the_folder_changes_nothing() {
        assert_eq!(
            decide(true, [File::Missing; 2], [File::Missing, ours(1)]),
            Decision::NoSource(0)
        );
        assert_eq!(
            decide(true, [File::Missing; 2], [ours(0), File::Other]),
            Decision::NoSource(1)
        );
    }

    #[test]
    fn off_removes_ours_only() {
        let source = [ours(0), ours(1)];
        assert_eq!(
            decide(false, [ours(0), ours(1)], source),
            Decision::Remove(vec![0, 1])
        );
        assert_eq!(
            decide(false, [File::Other, ours(1)], source),
            Decision::Remove(vec![1])
        );
        assert_eq!(
            decide(false, [File::Other, File::Other], source),
            Decision::Nothing
        );
        assert_eq!(
            decide(false, [File::Missing; 2], [File::Missing; 2]),
            Decision::Nothing
        );
    }
}
