// Hashing for verified copies (A51): one `Hasher` over BLAKE3 or SHA-256, and the `Manifest` that
// folds every verified file's digest into the one value a job records.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use sha2::Digest as _;

use crate::model::{Verification, VerifyAlgorithm};

/// An incremental hash in one of the algorithms verification offers.
pub enum Hasher {
    Blake3(Box<blake3::Hasher>),
    Sha256(Box<sha2::Sha256>),
}

impl Hasher {
    pub fn new(algorithm: VerifyAlgorithm) -> Self {
        match algorithm {
            VerifyAlgorithm::Blake3 => Hasher::Blake3(Box::new(blake3::Hasher::new())),
            VerifyAlgorithm::Sha256 => Hasher::Sha256(Box::new(sha2::Sha256::new())),
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        match self {
            Hasher::Blake3(hasher) => {
                hasher.update(bytes);
            }
            Hasher::Sha256(hasher) => hasher.update(bytes),
        }
    }

    /// The digest of everything fed so far.
    pub fn finish(self) -> Vec<u8> {
        match self {
            Hasher::Blake3(hasher) => hasher.finalize().as_bytes().to_vec(),
            Hasher::Sha256(hasher) => hasher.finalize().to_vec(),
        }
    }
}

/// Lower-case hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

/// The digest of `bytes` in one go.
pub fn digest_of(algorithm: VerifyAlgorithm, bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Hasher::new(algorithm);
    hasher.update(bytes);
    hasher.finish()
}

/// Folds the digests of the files a job verified, in order, into one `Verification`. The result
/// depends on the digests and their order only, so it is the same for the same bytes however the
/// job was chunked.
pub struct Manifest {
    algorithm: VerifyAlgorithm,
    hasher: Hasher,
    files: u64,
}

impl Manifest {
    pub fn new(algorithm: VerifyAlgorithm) -> Self {
        Self {
            algorithm,
            hasher: Hasher::new(algorithm),
            files: 0,
        }
    }

    pub fn add(&mut self, file_digest: &[u8]) {
        self.hasher.update(file_digest);
        self.files += 1;
    }

    pub fn files(&self) -> u64 {
        self.files
    }

    /// What the manifest holds so far, or `None` when no file was verified. The manifest goes on
    /// working, so a failed job can report what it had verified up to the failure.
    pub fn snapshot(&self) -> Option<Verification> {
        if self.files == 0 {
            return None;
        }
        let digest = match &self.hasher {
            Hasher::Blake3(hasher) => hasher.finalize().as_bytes().to_vec(),
            Hasher::Sha256(hasher) => hasher.as_ref().clone().finalize().to_vec(),
        };
        Some(Verification {
            algorithm: self.algorithm,
            digest: hex(&digest),
            files: self.files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(
            hex(&digest_of(VerifyAlgorithm::Sha256, b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&digest_of(VerifyAlgorithm::Blake3, b"")),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn chunking_does_not_change_a_digest() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        for algorithm in [VerifyAlgorithm::Blake3, VerifyAlgorithm::Sha256] {
            let whole = digest_of(algorithm, &data);
            for chunk in [1usize, 7, 4096, 9_999] {
                let mut hasher = Hasher::new(algorithm);
                for part in data.chunks(chunk) {
                    hasher.update(part);
                }
                assert_eq!(hasher.finish(), whole, "{algorithm:?} {chunk}");
            }
        }
    }

    #[test]
    fn hex_is_lower_case_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xa0, 0xff]), "000fa0ff");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn a_manifest_depends_on_the_digests_and_their_order() {
        let a = digest_of(VerifyAlgorithm::Blake3, b"a");
        let b = digest_of(VerifyAlgorithm::Blake3, b"b");
        let make = |parts: &[&[u8]]| {
            let mut manifest = Manifest::new(VerifyAlgorithm::Blake3);
            for part in parts {
                manifest.add(part);
            }
            manifest.snapshot()
        };
        assert_eq!(make(&[]), None);
        let ab = make(&[&a, &b]).unwrap();
        assert_eq!(ab.files, 2);
        assert_eq!(make(&[&a, &b]).unwrap(), ab);
        assert_ne!(make(&[&b, &a]).unwrap().digest, ab.digest);
        assert_ne!(make(&[&a]).unwrap().digest, ab.digest);
        // A snapshot does not end the manifest.
        let mut manifest = Manifest::new(VerifyAlgorithm::Sha256);
        manifest.add(&a);
        let first = manifest.snapshot().unwrap();
        manifest.add(&b);
        assert_ne!(manifest.snapshot().unwrap().digest, first.digest);
        assert_eq!(manifest.files(), 2);
    }
}
