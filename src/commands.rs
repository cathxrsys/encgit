use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::Commands;
use crate::archive;
use crate::bundle::{EncGitBundle, deserialize_bundle};
use crate::crypto;
use crate::originalgit::Git;

fn setup_local_repo(repo_dir: &Path, remote_url: &str) -> Result<(Git, Git)> {
    let encgit_dir = repo_dir.join(".encgit");

    let user_git = Git::new(repo_dir);
    user_git.init().context("Failed to init repo")?;
    user_git
        .branch_set_main()
        .context("Failed to set main branch")?;
    fs::write(repo_dir.join(".gitignore"), ".encgit/\n").context("Failed to write .gitignore")?;

    fs::create_dir_all(&encgit_dir).context("Failed to create .encgit")?;
    let encgit = Git::new(&encgit_dir);
    encgit.init().context("Failed to init .encgit repo")?;
    encgit.branch_set_main().context("Failed to set branch")?;
    encgit
        .remote_add("origin", remote_url)
        .context("Failed to add remote")?;
    fs::write(encgit_dir.join(".gitignore"), "*\n!.data\n!.gitignore\n")
        .context("Failed to write .encgit/.gitignore")?;

    Ok((user_git, encgit))
}

fn repo_name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_string()
}

fn require_encgit_dir(workdir: &Path) -> Result<PathBuf> {
    let encgit_dir = workdir.join(".encgit");
    if !encgit_dir.exists() {
        bail!(".encgit not found. Are you inside an encgit repository?");
    }
    Ok(encgit_dir)
}

fn read_bundle_from_disk(encgit_dir: &Path) -> Result<EncGitBundle> {
    let bytes = fs::read(encgit_dir.join(".data")).context("Failed to read .data")?;
    deserialize_bundle(&bytes)
}

fn unique_staging_dir(parent: &Path, prefix: &str) -> Result<PathBuf> {
    for attempt in 0..100u32 {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System clock is before UNIX_EPOCH")?
            .as_nanos();
        let candidate = parent.join(format!(".{prefix}-{}-{stamp}-{attempt}", process::id()));
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to create staging dir {}", candidate.display())
                });
            }
        }
    }

    bail!(
        "Failed to allocate a unique staging directory in {}",
        parent.display()
    )
}

fn remove_staging_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove staging dir {}", path.display()))?;
    }
    Ok(())
}

fn confirm_force_pull() -> Result<bool> {
    print!(
        "{}",
        "Force pull will make the repository match the remote container and delete local files that are absent there. Continue? [y/N]: ".yellow()
    );
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("Failed to read confirmation")?;

    let answer = answer.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

pub(crate) fn run(command: Commands, workdir: &Path) -> Result<()> {
    match command {
        Commands::Init { repo } => {
            let name = repo_name_from_url(&repo);
            let repo_dir = workdir.join(&name);

            if repo_dir.exists() {
                bail!("directory '{}' already exists.", name);
            }

            let staging_dir = unique_staging_dir(workdir, "encgit-init")?;
            let result = (|| {
                let encgit_dir = staging_dir.join(".encgit");
                let (user_git, encgit) = setup_local_repo(&staging_dir, &repo)?;

                let remote_has_data = encgit.fetch_origin().is_ok()
                    && encgit.checkout_file("origin/main", ".data").is_ok();
                if remote_has_data {
                    bail!(
                        "remote already contains encrypted data. Use 'encgit clone {}' to download and decrypt it.",
                        repo
                    );
                }

                println!("{}", "Initializing new encrypted repository...".green());
                user_git.add_all().context("Failed to git add")?;
                user_git
                    .commit("encgit init")
                    .context("Failed to git commit")?;

                let encrypted = crypto::zip_and_encrypt(&staging_dir)?;
                fs::write(encgit_dir.join(".data"), encrypted).context("Failed to write .data")?;

                encgit.add_all().context("Failed to git add")?;
                encgit
                    .commit("encgit init")
                    .context("Failed to git commit")?;
                encgit
                    .push_force("origin", "main")
                    .context("Failed to push")?;

                Ok(())
            })();

            if let Err(error) = result {
                let _ = remove_staging_dir(&staging_dir);
                return Err(error);
            }

            fs::rename(&staging_dir, &repo_dir).with_context(|| {
                format!(
                    "Failed to move initialized repository into {}",
                    repo_dir.display()
                )
            })?;

            println!("{}", format!("Done! cd {} and use 'encgit push/pull'.", name).green());
        }

        Commands::Clone { repo } => {
            let name = repo_name_from_url(&repo);
            let repo_dir = workdir.join(&name);

            if repo_dir.exists() {
                bail!("directory '{}' already exists.", name);
            }

            let staging_dir = unique_staging_dir(workdir, "encgit-clone")?;
            let result = (|| {
                let encgit_dir = staging_dir.join(".encgit");
                let (_user_git, encgit) = setup_local_repo(&staging_dir, &repo)?;

                println!("{}", "Fetching encrypted repository...".green());
                encgit
                    .fetch_origin()
                    .context("Failed to fetch from remote")?;
                encgit.checkout_file("origin/main", ".data").context(
                    "Failed to checkout .data; remote may be empty, use 'encgit init' instead",
                )?;

                let bundle = read_bundle_from_disk(&encgit_dir)?;
                let plaintext = crypto::decrypt_payload(&bundle)?;

                archive::restore_exact_from_zip(plaintext.as_ref(), &staging_dir)?;

                Ok(())
            })();

            if let Err(error) = result {
                let _ = remove_staging_dir(&staging_dir);
                return Err(error);
            }

            fs::rename(&staging_dir, &repo_dir).with_context(|| {
                format!(
                    "Failed to move cloned repository into {}",
                    repo_dir.display()
                )
            })?;

            println!("{}", format!("Done! cd {} and use 'encgit push/pull'.", name).green());
        }

        Commands::Push => {
            let encgit_dir = require_encgit_dir(workdir)?;
            archive::validate_repo_gitignore(workdir)?;

            println!("{}", "Encrypting and pushing...".green());
            let encrypted = crypto::zip_and_encrypt(workdir)?;
            fs::write(encgit_dir.join(".data"), &encrypted).context("Failed to write .data")?;

            let encgit = Git::new(&encgit_dir);
            encgit.add_all().context("Failed to git add")?;
            encgit.commit_timestamp().context("Failed to git commit")?;
            encgit
                .push_force("origin", "main")
                .context("Failed to push")?;

            println!("{}", "Pushed successfully.".green());
        }

        Commands::Pull { force } => {
            let encgit_dir = require_encgit_dir(workdir)?;

            if force && !confirm_force_pull()? {
                bail!("Force pull cancelled by user");
            }

            println!("{}", "Fetching from remote...".green());
            let encgit = Git::new(&encgit_dir);
            encgit.fetch_origin().context("Failed to fetch")?;
            encgit
                .checkout_file("origin/main", ".data")
                .context("Failed to checkout .data")?;

            let bundle = read_bundle_from_disk(&encgit_dir)?;
            let plaintext = crypto::decrypt_payload(&bundle)?;

            if force {
                archive::restore_exact_from_zip(plaintext.as_ref(), workdir)?;
            } else {
                archive::unzip_to_dir(plaintext.as_ref(), workdir)?;
            }

            println!("{}", "Pulled and decrypted successfully.".green());
        }
    }

    Ok(())
}
