//! The server's answer to "which client files do you offer now"
//! (`GET /api/client-files` of the srp web server), and the file beside the
//! updater that remembers what was installed from which archive.
//!
//! Both are a few lines of text, a word and its values on each:
//!
//! ```text
//! skhid-client-files 1
//! sha256 <64 hex digits>      the archive at /download
//! size <bytes>
//! min-revision <0-255>        the least game revision the server lets in
//! ```
//!
//! `min-revision` (srp's `SRP_MIN_PATCH_REVISION`; servers before it do not
//! say it) is what the login server compares with the revision the game
//! exe says in its sign-in (`crate::revision`).
//!
//! ```text
//! skhid-installed 1
//! archive <64 hex digits>     the archive these files came from
//! file <64 hex digits> <bytes> <path in the archive>
//! ```
//!
//! Lines with a word this version does not know are skipped, so a later
//! server may say more. A first line with a higher number is a format this
//! updater cannot read: it then says so instead of guessing.

use alloc::string::String;
use alloc::vec::Vec;

use crate::sha256::{from_hex, to_hex};
use crate::zip::safe_name;

/// The most an archive may weigh: it is downloaded into memory by a 32-bit
/// process.
pub const MAX_ARCHIVE_SIZE: u64 = 256 << 20;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ManifestError {
    /// The first line is not this format's: another page answered (a proxy's
    /// error page, a captive portal).
    NotManifest,
    /// The format's number is higher than this updater knows.
    NewerFormat,
    /// A needed line is missing or its value cannot be read.
    Incomplete,
    /// The archive is larger than [`MAX_ARCHIVE_SIZE`].
    TooLarge,
}

/// What the server offers.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Manifest {
    pub sha256: [u8; 32],
    pub size: u64,
    /// The least game revision the server lets in; `None`: not said.
    pub min_revision: Option<u8>,
}

pub(crate) fn lines(text: &[u8]) -> impl Iterator<Item = &[u8]> {
    text.split(|&byte| byte == b'\n')
        .map(|line| line.strip_suffix(b"\r").unwrap_or(line))
}

pub(crate) fn word(line: &[u8]) -> (&[u8], &[u8]) {
    match line.iter().position(|&byte| byte == b' ') {
        Some(at) => (&line[..at], &line[at + 1..]),
        None => (line, &[]),
    }
}

pub(crate) fn number(text: &[u8]) -> Option<u64> {
    if text.is_empty() || text.len() > 18 || !text.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(
        text.iter()
            .fold(0, |value, digit| value * 10 + u64::from(digit - b'0')),
    )
}

/// The first line: `<magic> <format number>`.
fn header(line: Option<&[u8]>, magic: &[u8]) -> Result<(), ManifestError> {
    let (name, version) = word(line.ok_or(ManifestError::NotManifest)?);
    if name != magic {
        return Err(ManifestError::NotManifest);
    }
    match number(version) {
        Some(1) => Ok(()),
        Some(version) if version > 1 => Err(ManifestError::NewerFormat),
        _ => Err(ManifestError::NotManifest),
    }
}

/// Reads the server's answer.
pub fn parse_manifest(text: &[u8]) -> Result<Manifest, ManifestError> {
    let mut lines = lines(text);
    header(lines.next(), b"skhid-client-files")?;
    let (mut sha256, mut size, mut min_revision) = (None, None, None);
    for line in lines {
        match word(line) {
            (b"sha256", value) => sha256 = from_hex(value),
            (b"size", value) => size = number(value),
            // A value that cannot be read is not the server's answer.
            (b"min-revision", value) => {
                min_revision = Some(
                    number(value)
                        .and_then(|value| u8::try_from(value).ok())
                        .ok_or(ManifestError::Incomplete)?,
                );
            }
            _ => {}
        }
    }
    let (Some(sha256), Some(size)) = (sha256, size) else {
        return Err(ManifestError::Incomplete);
    };
    if size == 0 {
        return Err(ManifestError::Incomplete);
    }
    if size > MAX_ARCHIVE_SIZE {
        return Err(ManifestError::TooLarge);
    }
    Ok(Manifest {
        sha256,
        size,
        min_revision,
    })
}

/// A file that was installed, as the archive had it.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InstalledFile {
    pub name: String,
    pub sha256: [u8; 32],
    pub size: u64,
}

/// What is installed: the archive it came from and its files.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Installed {
    pub archive: [u8; 32],
    pub files: Vec<InstalledFile>,
}

/// Reads the installed-state file. `None` for anything that is not a whole,
/// well-formed one: the caller then installs again, which is always safe.
pub fn parse_installed(text: &[u8]) -> Option<Installed> {
    let mut lines = lines(text);
    header(lines.next(), b"skhid-installed").ok()?;
    let mut archive = None;
    let mut files: Vec<InstalledFile> = Vec::new();
    let mut ended = false;
    for line in lines {
        match word(line) {
            (b"archive", value) => archive = Some(from_hex(value)?),
            (b"file", value) => {
                let (hash, rest) = word(value);
                let (size, name) = word(rest);
                let name = core::str::from_utf8(name).ok()?;
                if !safe_name(name) {
                    return None;
                }
                files.push(InstalledFile {
                    name: name.into(),
                    sha256: from_hex(hash)?,
                    size: number(size)?,
                });
            }
            // The last line: a file cut short by a full disk or a power
            // loss has none.
            (b"end", _) => ended = true,
            _ => {}
        }
    }
    if !ended || files.is_empty() {
        return None;
    }
    Some(Installed {
        archive: archive?,
        files,
    })
}

/// The installed-state file's text.
pub fn write_installed(installed: &Installed) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"skhid-installed 1\r\narchive ");
    out.extend_from_slice(&to_hex(&installed.archive));
    out.extend_from_slice(b"\r\n");
    for file in &installed.files {
        out.extend_from_slice(b"file ");
        out.extend_from_slice(&to_hex(&file.sha256));
        out.push(b' ');
        push_number(&mut out, file.size);
        out.push(b' ');
        out.extend_from_slice(file.name.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"end\r\n");
    out
}

/// `value` in decimal.
pub fn push_number(out: &mut Vec<u8>, value: u64) {
    let mut digits = [0_u8; 20];
    let mut at = digits.len();
    let mut rest = value;
    loop {
        at -= 1;
        digits[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    out.extend_from_slice(&digits[at..]);
}

/// A file a player is told to edit (the start scripts' language and server
/// lines) or may edit: text, and the updater's own `.url` (which server it
/// asks). Such a file that differs from the archive's is left alone until
/// the server offers another archive. Every other file is
/// program or certificate and must be the archive's byte for byte.
pub fn player_may_edit(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, extension)| {
        ["bat", "sh", "txt", "url"]
            .iter()
            .any(|text| extension.eq_ignore_ascii_case(text))
    })
}

/// What a look at one installed file found.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Found {
    /// There, with the recorded size and checksum.
    Same,
    /// There, with other bytes.
    Changed,
    Missing,
}

/// Why the archive is installed (again).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Reason {
    /// No readable installed-state file: the first run, or the file was lost.
    NothingInstalled,
    /// The server offers another archive than the one installed.
    NewArchive,
    /// A file of the installed archive is gone.
    Missing(String),
    /// A program file of the installed archive has other bytes.
    Changed(String),
}

/// Whether the files are those of the server's archive: `None` when they
/// are, else why not. `look` reports on one installed file.
pub fn why_install(
    installed: Option<&Installed>,
    offered: &Manifest,
    mut look: impl FnMut(&InstalledFile) -> Found,
) -> Option<Reason> {
    let Some(installed) = installed else {
        return Some(Reason::NothingInstalled);
    };
    if installed.archive != offered.sha256 {
        return Some(Reason::NewArchive);
    }
    damage(installed, &mut look)
}

/// What is wrong with the installed files themselves, whatever the server
/// offers: `None` when every one is there and no program file has changed.
pub fn damage(
    installed: &Installed,
    mut look: impl FnMut(&InstalledFile) -> Found,
) -> Option<Reason> {
    for file in &installed.files {
        match look(file) {
            Found::Same => {}
            Found::Changed if player_may_edit(&file.name) => {}
            Found::Changed => return Some(Reason::Changed(file.name.clone())),
            Found::Missing => return Some(Reason::Missing(file.name.clone())),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256::digest;

    fn manifest_text(sha: &[u8; 32], size: &str) -> Vec<u8> {
        let mut text = b"skhid-client-files 1\nsha256 ".to_vec();
        text.extend_from_slice(&to_hex(sha));
        text.extend_from_slice(b"\nsize ");
        text.extend_from_slice(size.as_bytes());
        text.push(b'\n');
        text
    }

    #[test]
    fn the_servers_answer_is_read() {
        let sha = digest(b"zip");
        let manifest = parse_manifest(&manifest_text(&sha, "4918126")).unwrap();
        assert_eq!(
            manifest,
            Manifest {
                sha256: sha,
                size: 4_918_126,
                min_revision: None,
            }
        );
        // CRLF, an unknown line and no last line end are all fine.
        let mut text =
            b"skhid-client-files 1\r\nnote later servers may say more\r\nsize 5\r\nsha256 "
                .to_vec();
        text.extend_from_slice(&to_hex(&sha));
        assert_eq!(
            parse_manifest(&text).unwrap(),
            Manifest {
                sha256: sha,
                size: 5,
                min_revision: None,
            }
        );
    }

    #[test]
    fn the_least_revision_let_in_is_read() {
        let sha = digest(b"zip");
        let with = |line: &str| {
            let mut text = manifest_text(&sha, "5");
            text.extend_from_slice(line.as_bytes());
            parse_manifest(&text)
        };
        assert_eq!(with("min-revision 27\n").unwrap().min_revision, Some(27));
        assert_eq!(with("min-revision 0").unwrap().min_revision, Some(0));
        assert_eq!(with("min-revision 255\n").unwrap().min_revision, Some(255));
        assert_eq!(with("").unwrap().min_revision, None);
        for bad in ["min-revision 256\n", "min-revision\n", "min-revision x\n"] {
            assert_eq!(with(bad), Err(ManifestError::Incomplete), "{bad}");
        }
    }

    #[test]
    fn other_answers_are_told_apart() {
        let sha = digest(b"zip");
        assert_eq!(parse_manifest(b""), Err(ManifestError::NotManifest));
        assert_eq!(
            parse_manifest(b"<!doctype html><html>"),
            Err(ManifestError::NotManifest)
        );
        assert_eq!(
            parse_manifest(b"skhid-client-files"),
            Err(ManifestError::NotManifest)
        );
        assert_eq!(
            parse_manifest(b"skhid-client-files x"),
            Err(ManifestError::NotManifest)
        );
        assert_eq!(
            parse_manifest(b"skhid-client-files 2\nfuture"),
            Err(ManifestError::NewerFormat)
        );
        assert_eq!(
            parse_manifest(b"skhid-client-files 1\nsize 5\n"),
            Err(ManifestError::Incomplete)
        );
        assert_eq!(
            parse_manifest(&manifest_text(&sha, "0")),
            Err(ManifestError::Incomplete)
        );
        assert_eq!(
            parse_manifest(&manifest_text(&sha, "-5")),
            Err(ManifestError::Incomplete)
        );
        assert_eq!(
            parse_manifest(&manifest_text(&sha, "12a")),
            Err(ManifestError::Incomplete)
        );
        assert_eq!(
            parse_manifest(&manifest_text(&sha, "99999999999999999999999")),
            Err(ManifestError::Incomplete)
        );
        assert_eq!(
            parse_manifest(&manifest_text(&sha, "268435457")),
            Err(ManifestError::TooLarge)
        );
        assert!(parse_manifest(&manifest_text(&sha, "268435456")).is_ok());
        let mut short = manifest_text(&sha, "5");
        short.remove(30);
        assert_eq!(parse_manifest(&short), Err(ManifestError::Incomplete));
    }

    fn installed() -> Installed {
        Installed {
            archive: digest(b"archive"),
            files: vec![
                InstalledFile {
                    name: "game/binaries/x86/survarium_skhid.exe".into(),
                    sha256: digest(b"exe"),
                    size: 12_895_352,
                },
                InstalledFile {
                    name: "game/binaries/x86/play_skhid.bat".into(),
                    sha256: digest(b"bat"),
                    size: 0,
                },
                InstalledFile {
                    name: "README.txt".into(),
                    sha256: digest(b"readme"),
                    size: 77,
                },
            ],
        }
    }

    #[test]
    fn the_installed_state_goes_to_text_and_back() {
        let state = installed();
        let text = write_installed(&state);
        assert_eq!(parse_installed(&text), Some(state.clone()));
        assert!(text.starts_with(b"skhid-installed 1\r\narchive "));
        assert!(text.ends_with(b"end\r\n"));
        // Cut anywhere, it is not a state: the install is then done again.
        for cut in 0..text.len() - 2 {
            assert_eq!(parse_installed(&text[..cut]), None, "cut at {cut}");
        }
    }

    #[test]
    fn a_state_file_that_was_tampered_with_is_no_state() {
        let text = String::from_utf8(write_installed(&installed())).unwrap();
        for (from, to) in [
            ("README.txt", "../README.txt"),
            ("README.txt", "c:/README.txt"),
            ("skhid-installed 1", "skhid-installed 2"),
            ("skhid-installed 1", "something else"),
            (" 77 ", " seventy "),
            ("archive ", "archive zz"),
        ] {
            assert!(text.contains(from));
            assert_eq!(
                parse_installed(text.replace(from, to).as_bytes()),
                None,
                "{from} -> {to}"
            );
        }
        assert_eq!(
            parse_installed(b"skhid-installed 1\r\narchive 00\r\nend\r\n"),
            None
        );
    }

    #[test]
    fn numbers_print_in_decimal() {
        for (value, text) in [
            (0, "0"),
            (7, "7"),
            (10, "10"),
            (4_918_126, "4918126"),
            (u64::MAX, "18446744073709551615"),
        ] {
            let mut out = Vec::new();
            push_number(&mut out, value);
            assert_eq!(out, text.as_bytes());
        }
    }

    #[test]
    fn an_install_is_due_for_the_right_reasons_only() {
        let state = installed();
        let same = Manifest {
            sha256: state.archive,
            size: 1,
            min_revision: None,
        };
        let other = Manifest {
            sha256: digest(b"other"),
            size: 1,
            min_revision: None,
        };
        let all_same = |_: &InstalledFile| Found::Same;
        assert_eq!(
            why_install(None, &same, all_same),
            Some(Reason::NothingInstalled)
        );
        assert_eq!(why_install(Some(&state), &same, all_same), None);
        assert_eq!(
            why_install(Some(&state), &other, all_same),
            Some(Reason::NewArchive)
        );
        // A missing file of any kind.
        let missing = |name: &'static str| {
            move |file: &InstalledFile| {
                if file.name == name {
                    Found::Missing
                } else {
                    Found::Same
                }
            }
        };
        assert_eq!(
            why_install(Some(&state), &same, missing("README.txt")),
            Some(Reason::Missing("README.txt".into()))
        );
        // A changed program file, but not a start script a player edited.
        let changed = |name: &'static str| {
            move |file: &InstalledFile| {
                if file.name.ends_with(name) {
                    Found::Changed
                } else {
                    Found::Same
                }
            }
        };
        assert_eq!(
            why_install(Some(&state), &same, changed("survarium_skhid.exe")),
            Some(Reason::Changed(
                "game/binaries/x86/survarium_skhid.exe".into()
            ))
        );
        assert_eq!(
            why_install(Some(&state), &same, changed("play_skhid.bat")),
            None
        );
        assert_eq!(
            why_install(Some(&state), &same, changed("README.txt")),
            None
        );
        assert_eq!(damage(&state, changed("play_skhid.bat")), None);
        assert!(damage(&state, missing("game/binaries/x86/play_skhid.bat")).is_some());
    }

    #[test]
    fn text_files_are_the_players_to_edit() {
        for name in [
            "play_skhid.bat",
            "a/play_skhid.sh",
            "README.txt",
            "LINUX.TXT",
            "updater_skhid.url",
        ] {
            assert!(player_may_edit(name), "{name}");
        }
        for name in [
            "survarium_skhid.exe",
            "bugtrap.dll",
            "a/b.crt",
            "bat",
            "x.bat.exe",
        ] {
            assert!(!player_may_edit(name), "{name}");
        }
    }
}
