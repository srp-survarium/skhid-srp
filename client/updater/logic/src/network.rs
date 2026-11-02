//! The update server, or every address of a file, could not be reached:
//! which of the usual ways it failed, by WinHTTP's error code. The player is
//! told in the box of `crate::problem` (no connection, or a server this
//! network blocks); [`english`] is the log's sentence, and the launcher's
//! line, which is ASCII only.

use alloc::format;
use alloc::string::String;

/// WinHTTP's codes (`winhttp.h`).
pub const ERROR_WINHTTP_TIMEOUT: u32 = 12002;
pub const ERROR_WINHTTP_NAME_NOT_RESOLVED: u32 = 12007;
pub const ERROR_WINHTTP_CANNOT_CONNECT: u32 = 12029;
pub const ERROR_WINHTTP_CONNECTION_ERROR: u32 = 12030;

/// How the server could not be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    /// The name was not found (DNS).
    NotFound,
    /// The connection was refused (or reset at once).
    Refused,
    /// No answer in time.
    Timeout,
    /// The connection was cut off after it was made.
    CutOff,
}

impl Failure {
    /// The failure a WinHTTP error code is; `None` for any other error.
    pub fn of(code: u32) -> Option<Self> {
        match code {
            ERROR_WINHTTP_NAME_NOT_RESOLVED => Some(Self::NotFound),
            ERROR_WINHTTP_CANNOT_CONNECT => Some(Self::Refused),
            ERROR_WINHTTP_TIMEOUT => Some(Self::Timeout),
            ERROR_WINHTTP_CONNECTION_ERROR => Some(Self::CutOff),
            _ => None,
        }
    }
}

/// What happened, in English (the log's, and the launcher's line).
pub fn english(failure: Failure, host: &str) -> String {
    match failure {
        Failure::NotFound => format!("Couldn't find {host}."),
        Failure::Refused => format!("{host} refused the connection."),
        Failure::Timeout => format!("{host} didn't answer."),
        Failure::CutOff => format!("The connection to {host} was cut off."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winhttp_codes_are_told_apart() {
        assert_eq!(Failure::of(12007), Some(Failure::NotFound));
        assert_eq!(Failure::of(12029), Some(Failure::Refused));
        assert_eq!(Failure::of(12002), Some(Failure::Timeout));
        assert_eq!(Failure::of(12030), Some(Failure::CutOff));
        // A certificate, a proxy, anything else: not a blocked server.
        for code in [0, 5, 12175, 12038, 12045, 12057] {
            assert_eq!(Failure::of(code), None, "{code}");
        }
    }

    #[test]
    fn the_english_sentences() {
        assert_eq!(
            english(Failure::NotFound, "skhid.link"),
            "Couldn't find skhid.link."
        );
        assert_eq!(
            english(Failure::Refused, "skhid.link"),
            "skhid.link refused the connection."
        );
        assert_eq!(
            english(Failure::Timeout, "skhid.link"),
            "skhid.link didn't answer."
        );
        assert_eq!(
            english(Failure::CutOff, "skhid.link"),
            "The connection to skhid.link was cut off."
        );
    }
}
