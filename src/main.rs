use anyhow::{Context, Result};
use colored::Colorize;
use clap::{Parser, Subcommand};

mod archive;
mod banner;
mod bundle;
mod commands;
mod crypto;
mod kdf;
mod originalgit;

#[derive(Parser)]
#[command(name = "encgit")]
#[command(version = "1.0.0 [beta]")]
#[command(about = "Encrypted git wrapper")]
struct Cli {
    #[arg(long, global = true)]
    workdir: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Initialize a new empty encrypted repository and push to remote
    Init { repo: String },

    /// Clone an existing encrypted repository and decrypt it locally
    Clone { repo: String },

    /// Encrypt local repo and push to remote
    Push,

    /// Pull encrypted data from remote and decrypt
    Pull {
        #[arg(long)]
        force: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{}", format!("Error: {error:#}").red());
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || args.contains(&"--help".to_string()) {
        banner::print_banner();
    } else {
        banner::print_mini_banner();
    }

    let cli = Cli::parse();
    let base = std::env::current_dir().context("Failed to get current directory")?;
    let workdir = match &cli.workdir {
        Some(path) => base
            .join(path)
            .canonicalize()
            .with_context(|| format!("Invalid --workdir: {path}"))?,
        None => base,
    };

    commands::run(cli.command, &workdir)
}
