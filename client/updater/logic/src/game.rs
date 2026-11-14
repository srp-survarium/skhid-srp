//! Release pins: changing a mirror never changes which bytes are accepted.
use crate::sha256::from_hex;

/// What the game's archive is called in the release's list
/// (`release/game.tsv`) and in the mirrors srp offers more of
/// (`mirrors.tsv`); its addresses come from the server's `GET /game`, or
/// from the list built into the updater when the server cannot be reached.
pub const ARCHIVE_NAME: &str = "survarium-026g.7z";
/// The extractor is a file of the Skhid archive, beside the updater. It is
/// run only when it is the pinned file, as the downloaded one would be.
pub const EXTRACTOR_NAME: &str = "7zr.exe";

#[derive(Clone, Copy)]
pub struct Pin {
    pub size: u64,
    pub sha256: &'static str,
}

impl Pin {
    pub fn matches(self, size: u64, hash: [u8; 32]) -> bool {
        size == self.size && from_hex(self.sha256.as_bytes()) == Some(hash)
    }
}

// Downloaded from the user's Drive mirror; its SHA-1 independently matches
// archive.org's bb30b71fab155b3435917690fd2be87984546163 (build 2777).
pub const ARCHIVE: Pin = Pin {
    size: 2_906_544_103,
    sha256: "7435558eeb17098fc9337c4ba9470402cae4c9be974ef5d9bc544f19b122a028",
};
// The official standalone 7zr.exe of 7-Zip 26.04 (www.7-zip.org/a/7zr.exe,
// fetched over HTTPS on 2026-10-13); the copy in ../../extractor.
pub const EXTRACTOR: Pin = Pin {
    size: 602_624,
    sha256: "256feca8e274e5da655e2a284fabafd9f554365eb164862089dacd4e8276d282",
};
pub const ORIGINAL_EXE: Pin = Pin {
    size: 12_894_840,
    sha256: "ac33d9937015c2299bf62cc6b127cb308b48a3475b30fff2ddd65479ae3bf153",
};

/// Only the files needed to run the game, relative to `Survarium/game`,
/// with build 2777's sizes and SHA-256 (the db's: `release/lib.sh`'s
/// `STOCK_DB_SHA256`; the others: `release/build-torrent`'s). Old vendor
/// installers, services and updaters are never extracted or run. The
/// archive's download checks the archive and the exe; one by torrent checks
/// each file by its pin (`exe/src/game.rs`).
pub const FILES: &[(&str, Pin)] = &[
    ("binaries/x86/survarium.exe", ORIGINAL_EXE),
    (
        "binaries/x86/bugtrap.dll",
        Pin {
            size: 278_648,
            sha256: "5bb81aea946767ad860522fc638b267e7e972c1e3d8af7a390c1714c1e9a8aac",
        },
    ),
    (
        "resources/ssl/survarium_login_server.crt",
        Pin {
            size: 757,
            sha256: "f29e488d406815c66a4541599c3bb8816a623dddcd995b588c902074ae8c7b68",
        },
    ),
    (
        "resources/sources/splash_screens/development_engine.bmp",
        Pin {
            size: 536_056,
            sha256: "f81222a3463da4cc5e8af193cf41f15199368b44f1938f6a2bad6107c1aac00f",
        },
    ),
    (
        "resources.db",
        Pin {
            size: 6_285_533_906,
            sha256: "c74078be702d17113b9ca23f59aa2b0087238481e7f01fb793620d51d875492c",
        },
    ),
];

/// How far the extractor is, 0 to 100, from what it printed (its `-bsp1`
/// progress): the last "NN%" in `output`.
pub fn extracted_percent(output: &[u8]) -> Option<u8> {
    output
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, byte)| **byte == b'%')
        .find_map(|(at, _)| {
            let digits = output[..at]
                .iter()
                .rev()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            core::str::from_utf8(&output[at - digits..at])
                .ok()?
                .parse::<u8>()
                .ok()
                .filter(|percent| *percent <= 100)
        })
}

/// The lines of what the extractor printed as a console would show them:
/// its progress rewrites a line over and over with `\r` (`\b` in other
/// builds), and only the last text of each line is kept.
pub fn output_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter_map(|line| {
        line.split(['\r', '\u{8}'])
            .map(str::trim)
            .rev()
            .find(|part| !part.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sha256::digest;

    #[test]
    fn size_and_sha256_are_both_mandatory() {
        let pin = Pin {
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert!(pin.matches(3, digest(b"abc")));
        assert!(!pin.matches(3, digest(b"abd"))); // same-length corruption
        assert!(!pin.matches(2, digest(b"abc"))); // truncated transfer
        assert!(!pin.matches(4, digest(b"abc"))); // excess bytes
        assert!(!Pin { sha256: "", ..pin }.matches(3, digest(b"abc")));
        assert!(
            !Pin {
                sha256: "not a checksum",
                ..pin
            }
            .matches(3, digest(b"abc"))
        );
    }

    #[test]
    fn release_pins_and_paths_are_well_formed() {
        for pin in [ARCHIVE, EXTRACTOR, ORIGINAL_EXE] {
            assert!(pin.size > 0);
            assert!(from_hex(pin.sha256.as_bytes()).is_some());
        }
        for (path, pin) in FILES {
            assert!(crate::zip::safe_name(path));
            assert!(pin.size > 0);
            assert!(from_hex(pin.sha256.as_bytes()).is_some());
        }
        assert!(crate::zip::safe_name(ARCHIVE_NAME));
        assert!(crate::zip::safe_name(EXTRACTOR_NAME));
    }

    // Printed with `-bsp1 -bb0 -sccUTF-8`: 7zr.exe 26.04 (under Wine) and,
    // with backspaces, the macOS 7zz 26.05.
    const WINDOWS: &[u8] = b"Blocks = 1\r\n\r\n  0%\r    \r 11%\r    \r 25%\r    \r 96%\r    \r100% 1\r        \rEverything is Ok\r\n\r\nSize:       250000000\r\nCompressed: 79490477\r\n";
    const MAC: &[u8] = b"Blocks = 2\n\n  0%\x08\x08\x08\x08    \x08\x08\x08\x08 18% - Survarium/game/resources.db\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08                                  \x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08 38% - Survarium/game/resources.db\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08                                  \x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08\x08Everything is Ok\n\nFiles: 2\n";

    #[test]
    fn the_extractors_last_percentage_is_read() {
        assert_eq!(extracted_percent(WINDOWS), Some(100));
        assert_eq!(extracted_percent(&WINDOWS[..30]), Some(11));
        assert_eq!(extracted_percent(MAC), Some(38));
        assert_eq!(extracted_percent(&MAC[..100]), Some(18));
        // Nothing yet, a percentage being written, none that is one.
        assert_eq!(extracted_percent(b"Blocks = 1\r\n\r\n"), None);
        assert_eq!(extracted_percent(b"  0%\r    \r 1"), Some(0));
        assert_eq!(extracted_percent(b"%\r 250%\r"), None);
    }

    #[test]
    fn the_log_gets_the_extractors_lines_without_its_progress() {
        let lines = |output| {
            output_lines(core::str::from_utf8(output).unwrap()).collect::<alloc::vec::Vec<_>>()
        };
        assert_eq!(
            lines(WINDOWS),
            [
                "Blocks = 1",
                "Everything is Ok",
                "Size:       250000000",
                "Compressed: 79490477"
            ]
        );
        assert_eq!(lines(MAC), ["Blocks = 2", "Everything is Ok", "Files: 2"]);
        // Cut short: how far it got.
        assert_eq!(lines(&WINDOWS[..30]), ["Blocks = 1", "11%"]);
    }
}
