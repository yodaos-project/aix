use alloc::{format, string::String, vec::Vec};
use core::fmt;

use ed25519_dalek::{Signer, Verifier};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const METADATA_PREFIX: &str = "META-INF/aix/";
pub const MANIFEST_PATH: &str = "META-INF/aix/manifest.json";
pub const SIGNATURE_PATH: &str = "META-INF/aix/signature.ed25519";
pub const PUBLIC_KEY_PATH: &str = "META-INF/aix/public-key.ed25519";
const DOMAIN: &[u8] = b"AIX-SIGNATURE\0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    InvalidPublicKey,
    InvalidSignature,
    InvalidEngineRange,
    InvalidEngineVersion,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPublicKey => "invalid Ed25519 public key",
            Self::InvalidSignature => "invalid Ed25519 signature",
            Self::InvalidEngineRange => "invalid engine version range",
            Self::InvalidEngineVersion => "invalid engine version",
        })
    }
}

pub struct PrivateKey(ed25519_dalek::SigningKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(ed25519_dalek::VerifyingKey);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

impl PrivateKey {
    pub fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        Self(ed25519_dalek::SigningKey::generate(rng))
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(ed25519_dalek::SigningKey::from_bytes(&seed))
    }

    pub fn to_seed_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    pub fn sign(&self, context: &[u8], message: &[u8]) -> Signature {
        Signature(self.0.sign(&signing_message(context, message)))
    }
}

impl PublicKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CryptoError> {
        ed25519_dalek::VerifyingKey::from_bytes(&bytes)
            .map(Self)
            .map_err(|_| CryptoError::InvalidPublicKey)
    }

    pub fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }

    pub fn key_id(self) -> String {
        format!("sha256:{}", hex(&Sha256::digest(self.to_bytes())))
    }

    pub fn verify(
        &self,
        context: &[u8],
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), CryptoError> {
        self.0
            .verify(&signing_message(context, message), &signature.0)
            .map_err(|_| CryptoError::InvalidSignature)
    }
}

impl Signature {
    pub fn from_bytes(bytes: [u8; 64]) -> Self {
        Self(ed25519_dalek::Signature::from_bytes(&bytes))
    }

    pub fn to_bytes(self) -> [u8; 64] {
        self.0.to_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageManifest {
    pub format: String,
    pub version: String,
    /// Stable project identity, independent of the per-build version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub engine: String,
    pub algorithm: String,
    pub digest: String,
    pub key_id: String,
    pub package_id: String,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

/// Stable project identities use a portable, normalized identifier alphabet.
pub fn is_valid_agent_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && !id.ends_with('-')
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationReport {
    pub package_id: String,
    pub version: String,
    pub engine: String,
    pub key_id: String,
    pub entry_count: usize,
}

pub fn engine_satisfies(range: &str, current_version: &str) -> Result<bool, CryptoError> {
    let normalized = normalize_range(range);
    let requirement =
        semver::VersionReq::parse(&normalized).map_err(|_| CryptoError::InvalidEngineRange)?;
    let version =
        semver::Version::parse(current_version).map_err(|_| CryptoError::InvalidEngineVersion)?;
    Ok(requirement.matches(&version))
}

pub fn validate_engine_range(range: &str) -> Result<(), CryptoError> {
    semver::VersionReq::parse(&normalize_range(range))
        .map(|_| ())
        .map_err(|_| CryptoError::InvalidEngineRange)
}

pub fn sha256(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

pub fn calculate_package_id(entries: &[ManifestEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update((entry.path.len() as u32).to_be_bytes());
        hasher.update(entry.path.as_bytes());
        hasher.update(entry.size.to_be_bytes());
        hasher.update((entry.sha256.len() as u32).to_be_bytes());
        hasher.update(entry.sha256.as_bytes());
    }
    format!("sha256:{}", hex(&hasher.finalize()))
}

fn normalize_range(range: &str) -> String {
    let trimmed = range.trim();
    if semver::Version::parse(trimmed).is_ok() {
        format!("={}", trimmed)
    } else {
        trimmed.into()
    }
}

fn signing_message(context: &[u8], message: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(DOMAIN.len() + 12 + context.len() + message.len());
    output.extend_from_slice(DOMAIN);
    output.extend_from_slice(&(context.len() as u32).to_be_bytes());
    output.extend_from_slice(context);
    output.extend_from_slice(&(message.len() as u64).to_be_bytes());
    output.extend_from_slice(message);
    output
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{:02x}", byte);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_and_signature_encodings_and_error_messages_are_stable() {
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;

        let mut expected_seed = [0; 32];
        ChaCha20Rng::from_seed([7; 32]).fill_bytes(&mut expected_seed);
        let key = PrivateKey::generate(&mut ChaCha20Rng::from_seed([7; 32]));
        assert_eq!(key.to_seed_bytes(), expected_seed);
        let public = key.public_key();
        assert_eq!(PublicKey::from_bytes(public.to_bytes()).unwrap(), public);
        let signature = key.sign(b"context", b"message");
        assert_eq!(Signature::from_bytes(signature.to_bytes()), signature);
        assert!(public
            .verify(b"other context", b"message", &signature)
            .is_err());
        assert!(PublicKey::from_bytes([2; 32]).is_err());
        for (error, message) in [
            (CryptoError::InvalidPublicKey, "invalid Ed25519 public key"),
            (CryptoError::InvalidSignature, "invalid Ed25519 signature"),
            (
                CryptoError::InvalidEngineRange,
                "invalid engine version range",
            ),
            (CryptoError::InvalidEngineVersion, "invalid engine version"),
        ] {
            assert_eq!(format!("{error}"), message);
        }
        assert_eq!(
            engine_satisfies("invalid", "1.0.0"),
            Err(CryptoError::InvalidEngineRange)
        );
        assert_eq!(
            engine_satisfies("*", "invalid"),
            Err(CryptoError::InvalidEngineVersion)
        );
    }

    #[test]
    fn signs_and_verifies() {
        let key = PrivateKey::from_seed([7; 32]);
        let signature = key.sign(b"test", b"payload");
        assert!(key
            .public_key()
            .verify(b"test", b"payload", &signature)
            .is_ok());
        assert!(key
            .public_key()
            .verify(b"test", b"changed", &signature)
            .is_err());
    }

    #[test]
    fn supports_expected_engine_ranges() {
        assert!(engine_satisfies("*", "99.0.0").unwrap());
        assert!(engine_satisfies("0.14.0", "0.14.0").unwrap());
        assert!(!engine_satisfies("0.14.0", "0.14.1").unwrap());
        assert!(engine_satisfies(">=0.14.0", "1.0.0").unwrap());
        assert!(engine_satisfies("^0.14.0", "0.14.9").unwrap());
        assert!(!engine_satisfies("^0.14.0", "0.15.0").unwrap());
    }
}
