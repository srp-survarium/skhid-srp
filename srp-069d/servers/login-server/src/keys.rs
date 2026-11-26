//! The login RSA key: RSA-1024, PKCS#1 v1.5, public key sent as DER RSAPublicKey in 0x0D
//! (the client stores it in a 160-byte buffer, so 1024 bits; 2048 bits overflow it).
//! The real private key is never in the repository: the path comes from the srp config (`files.login_key`).

use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, EncodeRsaPublicKey, LineEnding};
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};
use std::fmt;
use std::path::Path;

/// The client's buffer for the key DER (0x20011EED0 path).
pub const MAX_PUBLIC_DER: usize = 160;

#[derive(Debug)]
pub enum KeyError {
    Io(std::io::Error),
    Parse(String),
    /// Public key DER longer than the client's buffer.
    TooBig(usize),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::Io(e) => write!(f, "cannot read the login key: {e}"),
            KeyError::Parse(e) => write!(f, "cannot parse the login key: {e}"),
            KeyError::TooBig(n) => write!(f, "public key DER is {n} bytes; the client takes {MAX_PUBLIC_DER}"),
        }
    }
}

impl std::error::Error for KeyError {}

pub struct LoginKey {
    private: RsaPrivateKey,
    public_der: Vec<u8>,
}

impl LoginKey {
    pub fn new(private: RsaPrivateKey) -> Result<Self, KeyError> {
        let der = RsaPublicKey::from(&private)
            .to_pkcs1_der()
            .map_err(|e| KeyError::Parse(e.to_string()))?
            .as_bytes()
            .to_vec();
        if der.len() > MAX_PUBLIC_DER {
            return Err(KeyError::TooBig(der.len()));
        }
        Ok(Self { private, public_der: der })
    }

    /// Load a PEM private key (PKCS#1 `BEGIN RSA PRIVATE KEY` or PKCS#8).
    pub fn from_pem(pem: &str) -> Result<Self, KeyError> {
        let key = RsaPrivateKey::from_pkcs1_pem(pem)
            .or_else(|_| RsaPrivateKey::from_pkcs8_pem(pem))
            .map_err(|e| KeyError::Parse(e.to_string()))?;
        Self::new(key)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, KeyError> {
        Self::from_pem(&std::fs::read_to_string(path).map_err(KeyError::Io)?)
    }

    /// A fresh RSA-1024 key (the size the client takes), for an operator's first start (`srp setup` / `srp run` write it
    /// with [`LoginKey::write_new`]).
    pub fn generate() -> Result<Self, KeyError> {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 1024).map_err(|e| KeyError::Parse(e.to_string()))?;
        Self::new(key)
    }

    /// Generate a key and write it to `path` as a PKCS#1 PEM (`BEGIN RSA PRIVATE KEY`), readable by its owner only on
    /// unix. Never overwrites an existing file.
    pub fn write_new(path: impl AsRef<Path>) -> Result<Self, KeyError> {
        let path = path.as_ref();
        let key = Self::generate()?;
        let pem = key.private.to_pkcs1_pem(LineEnding::LF).map_err(|e| KeyError::Parse(e.to_string()))?;
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(KeyError::Io)?;
        }
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        use std::io::Write;
        o.open(path).and_then(|mut f| f.write_all(pem.as_bytes())).map_err(KeyError::Io)?;
        Ok(key)
    }

    /// A fresh key, for tests and local dev only.
    pub fn generate_for_tests(bits: usize) -> Self {
        Self::new(RsaPrivateKey::new(&mut rand::thread_rng(), bits).expect("key generation")).expect("size")
    }

    /// DER RSAPublicKey, as sent in 0x0D.
    pub fn public_der(&self) -> &[u8] {
        &self.public_der
    }

    /// CRC-32 of the public DER: the sign-in packet carries the CRC of the key the client holds
    /// (0 if none).
    pub fn public_crc32(&self) -> u32 {
        crc32fast::hash(&self.public_der)
    }

    pub fn public_key(&self) -> RsaPublicKey {
        RsaPublicKey::from(&self.private)
    }

    /// Ciphertext block size in bytes (128 for RSA-1024).
    pub fn block_size(&self) -> usize {
        self.private.size()
    }

    /// PKCS#1 v1.5 decryption of one block.
    pub fn decrypt(&self, block: &[u8]) -> Option<Vec<u8>> {
        self.private.decrypt(Pkcs1v15Encrypt, block).ok()
    }
}

/// What the client does with the public key of 0x0D (used by tests as the scripted client).
pub fn encrypt_with_public_der(public: &RsaPublicKey, plain: &[u8]) -> Vec<u8> {
    public.encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, plain).expect("plaintext fits one block")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_key_loads_again_and_fits_the_client() {
        let dir = std::env::temp_dir().join(format!("srp-key-test-{}", std::process::id()));
        let path = dir.join("sub").join("login_key.pem");
        let k = LoginKey::write_new(&path).unwrap();
        assert!(k.public_der().len() <= MAX_PUBLIC_DER);
        let again = LoginKey::from_file(&path).unwrap();
        assert_eq!(again.public_der(), k.public_der());
        assert!(LoginKey::write_new(&path).is_err(), "an existing key is never overwritten");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
