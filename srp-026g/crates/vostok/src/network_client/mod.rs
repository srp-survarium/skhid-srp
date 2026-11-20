mod tcp_client;

pub use tcp_client::{TcpClient, tune_connection};

use crate::serde::{Deserialize, DeserializeError, Serialize};

pub trait NetworkRequest: Deserialize {}
pub trait NetworkResponse: Serialize {}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Failed to deserialize the message: {0}")]
    DeserializeError(#[from] DeserializeError),
    #[error("Socket failure: {0}")]
    NetworkError(#[from] std::io::Error),
}

impl NetworkError {
    /// Whether the peer closed or reset the connection (a normal way for a
    /// client session to end) rather than sent something unreadable.
    pub fn is_disconnect(&self) -> bool {
        use std::io::ErrorKind;
        matches!(
            self,
            Self::NetworkError(error) if matches!(
                error.kind(),
                ErrorKind::UnexpectedEof
                    | ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
            ) || (cfg!(not(windows)) && error.kind() == ErrorKind::TimedOut)
        )
    }

    /// Whether a read gave up after the read timeout, with nothing lost.
    /// Off Windows a read timeout is `WouldBlock`, and `TimedOut` is the
    /// system giving up on a connection that went silent (see
    /// [`is_disconnect`](Self::is_disconnect)); Windows reports a read
    /// timeout as `TimedOut`.
    pub fn is_timeout(&self) -> bool {
        use std::io::ErrorKind;
        matches!(
            self,
            Self::NetworkError(error)
                if error.kind() == ErrorKind::WouldBlock
                    || (cfg!(windows) && error.kind() == ErrorKind::TimedOut)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    fn error(kind: ErrorKind) -> NetworkError {
        Error::from(kind).into()
    }

    #[test]
    fn a_read_timeout_is_told_from_a_connection_the_system_gave_up_on() {
        assert!(error(ErrorKind::WouldBlock).is_timeout());
        assert!(!error(ErrorKind::WouldBlock).is_disconnect());
        // Off Windows a `TimedOut` is the system giving up (keepalive, or
        // unacknowledged data): the connection is gone, not merely quiet.
        // Windows reports its read timeout this way, so it is a timeout there.
        assert_eq!(error(ErrorKind::TimedOut).is_timeout(), cfg!(windows));
        assert_eq!(
            error(ErrorKind::TimedOut).is_disconnect(),
            cfg!(not(windows))
        );
        assert!(error(ErrorKind::ConnectionReset).is_disconnect());
    }
}
