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

/// WinHTTP's codes of a connection that is not secure (`winhttp.h`): TLS
/// that failed, a certificate this PC does not take. A clock that is wrong
/// makes every one of them, at every HTTPS address.
const INSECURE: [u32; 8] = [
    12037, // ERROR_WINHTTP_SECURE_CERT_DATE_INVALID
    12038, // ERROR_WINHTTP_SECURE_CERT_CN_INVALID
    12045, // ERROR_WINHTTP_SECURE_INVALID_CA
    12057, // ERROR_WINHTTP_SECURE_CERT_REV_FAILED
    12157, // ERROR_WINHTTP_SECURE_CHANNEL_ERROR
    12169, // ERROR_WINHTTP_SECURE_INVALID_CERT
    12170, // ERROR_WINHTTP_SECURE_CERT_REVOKED
    12175, // ERROR_WINHTTP_SECURE_FAILURE
];

/// Whether a WinHTTP error code is of a connection that is not secure.
pub fn insecure(code: u32) -> bool {
    INSECURE.contains(&code)
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
        for code in [12037, 12038, 12045, 12057, 12157, 12169, 12170, 12175] {
            assert!(insecure(code), "{code}");
        }
        for code in [0, 5, 12002, 12007, 12029, 12030] {
            assert!(!insecure(code), "{code}");
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
