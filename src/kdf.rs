use anyhow::{Result, anyhow};
use argon2::{Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize, Clone)]
pub struct Argon2Config {
    pub mem_cost: u32,
    pub time_cost: u32,
    pub lanes: u32,
}

impl Default for Argon2Config {
    fn default() -> Self {
        Self {
            mem_cost: 1024 * 256,
            time_cost: 64,
            lanes: 1,
        }
    }
}

pub fn derive_key(
    password: &str,
    salt: &[u8],
    config: &Argon2Config,
) -> Result<Zeroizing<Vec<u8>>> {
    let params = Params::new(config.mem_cost, config.time_cost, config.lanes, Some(32))
        .map_err(|error| anyhow!("Invalid Argon2 parameters: {error}"))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);

    let mut key = vec![0u8; 32];

    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| anyhow!("Argon2 key derivation failed: {error}"))?;

    Ok(Zeroizing::new(key))
}
