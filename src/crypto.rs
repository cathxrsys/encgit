use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use chacha20poly1305::aead::rand_core::RngCore;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rpassword::read_password;
use zeroize::Zeroizing;

use crate::archive;
use crate::bundle::{EncGitBundle, make_aad, serialize_bundle, validate_argon2_config};
use crate::kdf::{self, Argon2Config};
use crate::originalgit::Git;

fn read_secret(prompt: &str) -> Result<Zeroizing<String>> {
    print!("{prompt}");
    io::stdout().flush().context("Failed to flush stdout")?;
    read_password()
        .map(Zeroizing::new)
        .context("Failed to read password")
}

fn ask_password_for_encryption(salt: &[u8], argon2: &Argon2Config) -> Result<Zeroizing<Vec<u8>>> {
    let password = read_secret("Enter password: ")?;
    let confirmation = read_secret("Confirm password: ")?;
    if password.as_str() != confirmation.as_str() {
        bail!("Passwords do not match");
    }

    println!("{}", "Deriving key, please wait...".green());
    io::stdout().flush().context("Failed to flush stdout")?;
    kdf::derive_key(password.as_str(), salt, argon2)
}

fn ask_password_for_decryption(salt: &[u8], argon2: &Argon2Config) -> Result<Zeroizing<Vec<u8>>> {
    let password = read_secret("Enter password: ")?;
    println!("{}", "Deriving key, please wait...".green());
    io::stdout().flush().context("Failed to flush stdout")?;
    kdf::derive_key(password.as_str(), salt, argon2)
}

pub(crate) fn zip_and_encrypt(repo_dir: &Path) -> Result<Vec<u8>> {
    let git = Git::new(repo_dir);

    git.add_all()
        .context("Failed to git add local repository")?;
    git.commit_timestamp_if_needed()
        .context("Failed to git commit local repository")?;

    let zip_bytes = Zeroizing::new(archive::zip_directory(repo_dir)?);

    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let argon2 = Argon2Config::default();
    let aad = make_aad(&argon2, &salt);
    let key = ask_password_for_encryption(&salt, &argon2)?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: zip_bytes.as_ref(),
                aad: &aad,
            },
        )
        .map_err(|_| anyhow!("Encryption failed"))?;

    serialize_bundle(&EncGitBundle {
        argon2,
        salt,
        nonce,
        ciphertext,
    })
}

pub(crate) fn decrypt_payload(bundle: &EncGitBundle) -> Result<Zeroizing<Vec<u8>>> {
    validate_argon2_config(&bundle.argon2)?;

    let key = ask_password_for_decryption(&bundle.salt, &bundle.argon2)?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let aad = make_aad(&bundle.argon2, &bundle.salt);

    cipher
        .decrypt(
            Nonce::from_slice(&bundle.nonce),
            Payload {
                msg: bundle.ciphertext.as_ref(),
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| anyhow!("Decryption failed: wrong password or corrupted data"))
}
