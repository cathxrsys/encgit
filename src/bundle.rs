use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::kdf::Argon2Config;

#[derive(Serialize, Deserialize)]
pub(crate) struct EncGitBundle {
    pub(crate) argon2: Argon2Config,
    pub(crate) salt: [u8; 16],
    pub(crate) nonce: [u8; 12],
    pub(crate) ciphertext: Vec<u8>,
}

pub(crate) fn make_aad(argon2: &Argon2Config, salt: &[u8]) -> Vec<u8> {
    let mut aad = Vec::with_capacity(12 + salt.len());
    aad.extend_from_slice(&argon2.mem_cost.to_le_bytes());
    aad.extend_from_slice(&argon2.time_cost.to_le_bytes());
    aad.extend_from_slice(&argon2.lanes.to_le_bytes());
    aad.extend_from_slice(salt);
    aad
}

pub(crate) fn validate_argon2_config(config: &Argon2Config) -> Result<()> {
    if config.mem_cost < 8 || config.mem_cost > 16 * 1024 * 1024 {
        bail!(
            ".data contains invalid Argon2 mem_cost: {}",
            config.mem_cost
        );
    }
    if config.time_cost < 1 || config.time_cost > 4096 {
        bail!(
            ".data contains invalid Argon2 time_cost: {}",
            config.time_cost
        );
    }
    if config.lanes < 1 || config.lanes > 256 {
        bail!(".data contains invalid Argon2 lanes: {}", config.lanes);
    }
    Ok(())
}

pub(crate) fn serialize_bundle(bundle: &EncGitBundle) -> Result<Vec<u8>> {
    bincode::serialize(bundle).context("Failed to serialize .data bundle")
}

pub(crate) fn deserialize_bundle(bytes: &[u8]) -> Result<EncGitBundle> {
    bincode::deserialize(bytes).context("Failed to parse .data bundle")
}
