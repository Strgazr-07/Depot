//! Command-line surface. Designed so that, once installed (`cargo install
//! --path .`), `depot scan`, `depot` (TUI), and `depot hub …` all
//! work as first-class commands.

use std::collections::HashSet;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::scan::{default_root, Category, ScanOptions};

#[derive(Parser)]
#[command(
    name = "depot",
    version,
    about = "Reclaim developer disk space (npm / python / ML), plus a shared pnpm+uv package store.",
    long_about = None,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Launch the interactive TUI (the default with no subcommand).
    Tui(ScanArgs),
    /// Scan and print results — no UI. Add `--json` for machine output.
    Scan {
        #[command(flatten)]
        args: ScanArgs,
        /// Emit JSON instead of the grouped table.
        #[arg(long)]
        json: bool,
    },
    /// Assemble deps from the shared store (install only what's missing), then
    /// run a project script — no manual `install` step needed.
    Run {
        /// Script and its arguments, e.g. `dev` or `build --watch`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, value_name = "SCRIPT")]
        args: Vec<String>,
    },
    /// Assemble deps from the shared store without running anything.
    Ensure,
    /// Adopt the shared store for the current project (alias of `hub link`).
    Link,
    /// Shared pnpm + uv global package store (Feature 4).
    Hub {
        #[command(subcommand)]
        action: HubAction,
    },
}

/// Shared scan parameters used by both `scan` and `tui`.
#[derive(Args, Clone, Default)]
pub struct ScanArgs {
    /// Directory to scan (repeatable). Defaults to your home directory.
    #[arg(short, long = "path", value_name = "DIR")]
    pub paths: Vec<PathBuf>,

    /// Restrict to a category: node | python | model | build (repeatable).
    #[arg(short, long = "category", value_name = "CAT")]
    pub categories: Vec<String>,

    /// Ignore anything smaller than this many megabytes.
    #[arg(long, default_value_t = 0, value_name = "MB")]
    pub min_mb: u64,

    /// Scan every fixed drive instead of just your home directory.
    #[arg(long)]
    pub all_drives: bool,
}

impl ScanArgs {
    pub fn to_options(&self) -> ScanOptions {
        let roots = if self.all_drives {
            all_drives()
        } else if self.paths.is_empty() {
            vec![default_root()]
        } else {
            self.paths.clone()
        };

        let categories = if self.categories.is_empty() {
            None
        } else {
            Some(
                self.categories
                    .iter()
                    .filter_map(|c| Category::parse(c))
                    .collect::<HashSet<_>>(),
            )
        };

        ScanOptions {
            roots,
            categories,
            min_size: self.min_mb.saturating_mul(1024 * 1024),
        }
    }
}

#[derive(Subcommand)]
pub enum HubAction {
    /// Set up the shared global store and point pnpm + uv at it.
    Init,
    /// Add dependencies to the current project, routed through the shared store.
    Add {
        /// Package names to add (e.g. `lodash` or `requests`).
        #[arg(required = true)]
        packages: Vec<String>,
        /// Add as a dev dependency.
        #[arg(short = 'D', long)]
        dev: bool,
    },
    /// Show the store location, size, and registered projects.
    Status,
    /// Adopt the shared store for the current project (install through it).
    Link,
    /// Estimate how much the shared store saves across your projects.
    Analyze {
        /// Where to look for projects (repeatable). Defaults to home.
        #[arg(short, long = "path", value_name = "DIR")]
        paths: Vec<PathBuf>,
    },
}

#[cfg(windows)]
fn all_drives() -> Vec<PathBuf> {
    ('A'..='Z')
        .map(|c| PathBuf::from(format!("{c}:\\")))
        .filter(|p| p.is_dir())
        .collect()
}

#[cfg(not(windows))]
fn all_drives() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}
