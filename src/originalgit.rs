use anyhow::{Context, Result, anyhow};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Git {
    workdir: PathBuf,
}

impl Git {
    pub fn new<P: Into<PathBuf>>(workdir: P) -> Self {
        Self {
            workdir: workdir.into(),
        }
    }

    fn output(&self, args: &[&str]) -> Result<Output> {
        Command::new("git")
            .args(args)
            .current_dir(&self.workdir)
            .output()
            .with_context(|| format!("Failed to run git {}", args.join(" ")))
    }

    fn run(&self, args: &[&str]) -> Result<()> {
        let output = self.output(args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            if detail.is_empty() {
                return Err(anyhow!("git {} failed", args.join(" ")));
            }
            return Err(anyhow!("git {} failed: {}", args.join(" "), detail));
        }
        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        self.run(&["init"])
    }

    pub fn remote_add(&self, name: &str, url: &str) -> Result<()> {
        self.run(&["remote", "add", name, url])
    }

    pub fn branch_set_main(&self) -> Result<()> {
        self.run(&["branch", "-m", "main"])
    }

    pub fn add_all(&self) -> Result<()> {
        self.run(&["add", "."])
    }

    pub fn commit(&self, message: &str) -> Result<()> {
        self.run(&["commit", "-m", message])
    }

    fn timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    pub fn commit_timestamp(&self) -> Result<()> {
        self.run(&["commit", "-m", &Self::timestamp()])
    }

    pub fn has_changes(&self) -> Result<bool> {
        let output = self.output(&["status", "--porcelain"])?;
        Ok(!output.stdout.is_empty())
    }

    /// Like commit_timestamp but does nothing if there is nothing to commit.
    pub fn commit_timestamp_if_needed(&self) -> Result<()> {
        if self.has_changes()? {
            self.commit_timestamp()?;
        }
        Ok(())
    }

    pub fn push_force(&self, remote: &str, branch: &str) -> Result<()> {
        self.run(&["push", "-f", "-u", remote, branch])
    }

    pub fn fetch_origin(&self) -> Result<()> {
        self.run(&["fetch", "origin"])
    }

    /// Checkout a single file from a treeish (e.g. "origin/main").
    pub fn checkout_file(&self, treeish: &str, file: &str) -> Result<()> {
        self.run(&["checkout", treeish, "--", file])
    }
}
