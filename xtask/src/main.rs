//! Project automation: `cargo xtask <command>`. See `docs/PLAN.md`.

mod cpp;
mod inventory;
mod skia;

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "skia-rust project automation")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the pinned Skia checkout in `third_party/skia`.
    Skia {
        #[command(subcommand)]
        command: SkiaCommand,
    },
    /// Manage the test inventory (`inventory/manifest.toml`).
    Inventory {
        #[command(subcommand)]
        command: InventoryCommand,
    },
}

#[derive(Subcommand)]
enum SkiaCommand {
    /// Clone Skia at the pinned commit (shallow) into `third_party/skia`.
    Fetch,
}

#[derive(Subcommand)]
enum InventoryCommand {
    /// Re-scan the pinned Skia tree and update the manifest, preserving hand-edited fields.
    Sync {
        /// Skip the check that `third_party/skia` is at the pinned commit.
        #[arg(long)]
        allow_pin_mismatch: bool,
    },
    /// Print pass-rate statistics from the manifest.
    Stats,
}

fn main() -> Result<()> {
    let root = workspace_root();
    match Cli::parse().command {
        Command::Skia {
            command: SkiaCommand::Fetch,
        } => skia::fetch(&root),
        Command::Inventory { command } => match command {
            InventoryCommand::Sync { allow_pin_mismatch } => {
                inventory::sync(&root, allow_pin_mismatch)
            }
            InventoryCommand::Stats => inventory::stats(&root),
        },
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}
