//! Detection engine.
//!
//! [`run_scan`] streams [`ScanItem`]s back over an `mpsc` channel as they are
//! discovered. Detection comes from two places: a recursive [`walk`] of one or
//! more roots (node_modules, virtualenvs, build artifacts, scattered caches)
//! and probes of [`known`] fixed locations (package-manager caches, ML models,
//! conda environments).

pub mod known;
pub mod walk;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::SystemTime;

/// Top-level grouping used to organise the report and TUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Node,
    Python,
    Model,
    Build,
}

impl Category {
    /// Short machine-ish label (also used for `--category` filtering).
    pub fn label(&self) -> &'static str {
        match self {
            Category::Node => "node",
            Category::Python => "python",
            Category::Model => "model",
            Category::Build => "build",
        }
    }

    /// Human heading for grouped views.
    pub fn title(&self) -> &'static str {
        match self {
            Category::Node => "Node / JavaScript",
            Category::Python => "Python",
            Category::Model => "ML Models",
            Category::Build => "Build artifacts",
        }
    }

    pub const ALL: [Category; 4] = [
        Category::Node,
        Category::Python,
        Category::Model,
        Category::Build,
    ];

    /// Parse a `--category` value (accepts a few friendly aliases).
    pub fn parse(s: &str) -> Option<Category> {
        match s.trim().to_ascii_lowercase().as_str() {
            "node" | "js" | "javascript" | "npm" => Some(Category::Node),
            "python" | "py" => Some(Category::Python),
            "model" | "models" | "ml" => Some(Category::Model),
            "build" | "artifact" | "artifacts" => Some(Category::Build),
            _ => None,
        }
    }
}

/// A single removable thing the scanner found. `paths` holds one entry for a
/// normal item, or many for an aggregate (e.g. every `__pycache__` at once).
#[derive(Clone, Debug)]
pub struct ScanItem {
    pub category: Category,
    /// Specific kind, e.g. `node_modules`, `virtualenv`, `huggingface`.
    pub kind: String,
    /// Friendly display name (project name, model name, …).
    pub label: String,
    pub paths: Vec<PathBuf>,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl ScanItem {
    /// Representative path (the first); aggregates may have more.
    pub fn primary(&self) -> &Path {
        self.paths
            .first()
            .map(|p| p.as_path())
            .unwrap_or_else(|| Path::new(""))
    }

    pub fn count(&self) -> usize {
        self.paths.len()
    }
}

/// Messages streamed from the scan thread to the consumer (TUI or printer).
pub enum ScanMsg {
    Item(ScanItem),
    Status(String),
    Done,
}

/// What and where to scan.
#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub roots: Vec<PathBuf>,
    /// `None` means every category; otherwise only the listed ones.
    pub categories: Option<HashSet<Category>>,
    /// Items smaller than this (bytes) are dropped.
    pub min_size: u64,
}

impl ScanOptions {
    fn wants(&self, category: Category) -> bool {
        self.categories
            .as_ref()
            .map_or(true, |set| set.contains(&category))
    }
}

/// Default scan root: the user's home directory.
pub fn default_root() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Run a full scan, streaming results over `tx`. Blocks until finished, then
/// sends [`ScanMsg::Done`].
pub fn run_scan(opts: ScanOptions, tx: Sender<ScanMsg>) {
    let _ = tx.send(ScanMsg::Status("Checking caches, models & conda…".into()));
    known::scan_known(&opts, &tx);

    for root in &opts.roots {
        let _ = tx.send(ScanMsg::Status(format!("Scanning {}", root.display())));
        walk::walk_root(root, &opts, &tx);
    }

    let _ = tx.send(ScanMsg::Done);
}

/// Apply category / min-size filters, then forward the item. Central choke
/// point so every detector gets consistent filtering for free.
pub(crate) fn emit(tx: &Sender<ScanMsg>, opts: &ScanOptions, item: ScanItem) {
    if item.size == 0 || item.paths.is_empty() {
        return;
    }
    if !opts.wants(item.category) || item.size < opts.min_size {
        return;
    }
    let _ = tx.send(ScanMsg::Item(item));
}
