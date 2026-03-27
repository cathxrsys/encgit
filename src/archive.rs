use std::ffi::OsStr;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Component, Path};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use walkdir::WalkDir;

const ZIP_MAGIC: &[u8; 4] = &[0x50, 0x4B, 0x03, 0x04];

fn build_gitignore(dir: &Path) -> Result<Gitignore> {
    let gitignore_path = dir.join(".gitignore");
    let mut builder = GitignoreBuilder::new(dir);

    if gitignore_path.exists() {
        if let Some(error) = builder.add(&gitignore_path) {
            return Err(anyhow!(
                "Failed to parse {}: {}",
                gitignore_path.display(),
                error
            ));
        }
    }

    builder
        .build()
        .context("Failed to build .gitignore matcher")
}

fn is_top_level_path(relative: &Path, name: &str) -> bool {
    matches!(relative.components().next(), Some(Component::Normal(component)) if component == OsStr::new(name))
}

fn is_excluded(relative: &Path, is_dir: bool, matcher: &Gitignore) -> bool {
    if is_top_level_path(relative, ".git") || is_top_level_path(relative, ".encgit") {
        return true;
    }

    matcher.matched(relative, is_dir).is_ignore()
}

pub(crate) fn validate_repo_gitignore(dir: &Path) -> Result<()> {
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.is_file() {
        bail!(".gitignore not found in {}", dir.display());
    }

    let matcher = build_gitignore(dir)?;
    if !matcher.matched(Path::new(".encgit"), true).is_ignore() {
        bail!(".gitignore must ignore .encgit/");
    }

    Ok(())
}

pub(crate) fn zip_directory(dir: &Path) -> Result<Vec<u8>> {
    let matcher = build_gitignore(dir)?;
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut walker = WalkDir::new(dir).min_depth(1).into_iter();
    while let Some(entry) = walker.next() {
        let entry = entry.with_context(|| format!("Failed to walk {}", dir.display()))?;
        let path = entry.path();
        let relative = path
            .strip_prefix(dir)
            .with_context(|| format!("Failed to compute relative path for {}", path.display()))?;
        let is_dir = entry.file_type().is_dir();

        if is_excluded(relative, is_dir, &matcher) {
            if is_dir {
                walker.skip_current_dir();
            }
            continue;
        }

        let zip_path = relative
            .to_str()
            .ok_or_else(|| anyhow!("Repository contains non-UTF8 path: {}", relative.display()))?;

        if is_dir {
            zip.add_directory(zip_path, options).with_context(|| {
                format!("Failed to add directory {} to archive", relative.display())
            })?;
            continue;
        }

        zip.start_file(zip_path, options)
            .with_context(|| format!("Failed to start archive entry {}", relative.display()))?;
        let data = fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        zip.write_all(&data)
            .with_context(|| format!("Failed to write archive entry {}", relative.display()))?;
    }

    zip.finish()
        .context("Failed to finalize archive")
        .map(|writer| writer.into_inner())
}

pub(crate) fn unzip_to_dir(zip_bytes: &[u8], dir: &Path) -> Result<()> {
    if !zip_bytes.starts_with(ZIP_MAGIC) {
        bail!("Decrypted payload is not a valid zip archive");
    }

    let buffer = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(buffer).context("Invalid zip archive")?;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .with_context(|| format!("Failed to read archive entry #{index}"))?;
        let outpath = match file.enclosed_name() {
            Some(path) => dir.join(path),
            None => continue,
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory {}", outpath.display()))?;
            continue;
        }

        if let Some(parent) = outpath.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let mut outfile = fs::File::create(&outpath)
            .with_context(|| format!("Failed to create {}", outpath.display()))?;
        std::io::copy(&mut file, &mut outfile)
            .with_context(|| format!("Failed to extract {}", outpath.display()))?;
    }

    Ok(())
}

fn unique_temp_dir(parent: &Path, prefix: &str) -> Result<std::path::PathBuf> {
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
                return Err(error)
                    .with_context(|| format!("Failed to create temp dir {}", candidate.display()));
            }
        }
    }

    bail!(
        "Failed to allocate a unique temporary directory in {}",
        parent.display()
    )
}

fn remove_path(path: &Path) -> Result<()> {
    let file_type = fs::symlink_metadata(path)
        .with_context(|| format!("Failed to stat {}", path.display()))?
        .file_type();

    if file_type.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("Failed to remove directory {}", path.display()))?;
    } else {
        fs::remove_file(path)
            .with_context(|| format!("Failed to remove file {}", path.display()))?;
    }

    Ok(())
}

fn clear_directory_except(dir: &Path, keep: &[&str]) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("Failed to enumerate {}", dir.display()))?;
        let name = entry.file_name();
        if keep.iter().any(|item| name == OsStr::new(item)) {
            continue;
        }

        remove_path(&entry.path())?;
    }

    Ok(())
}

pub(crate) fn restore_exact_from_zip(zip_bytes: &[u8], dir: &Path) -> Result<()> {
    let parent = dir.parent().unwrap_or(dir);
    let temp_dir = unique_temp_dir(parent, "encgit-restore")?;

    let result = (|| {
        unzip_to_dir(zip_bytes, &temp_dir)?;
        clear_directory_except(dir, &[".git", ".encgit"])?;

        for entry in fs::read_dir(&temp_dir)
            .with_context(|| format!("Failed to read {}", temp_dir.display()))?
        {
            let entry =
                entry.with_context(|| format!("Failed to enumerate {}", temp_dir.display()))?;
            let target = dir.join(entry.file_name());
            fs::rename(entry.path(), &target).with_context(|| {
                format!("Failed to move restored entry into {}", target.display())
            })?;
        }

        Ok(())
    })();

    fs::remove_dir_all(&temp_dir)
        .with_context(|| format!("Failed to remove {}", temp_dir.display()))?;
    result
}
