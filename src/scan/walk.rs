//! Recursive walk of a root directory. Once a relevant directory is detected
//! it is recorded and *pruned* (we don't descend), so nested copies are never
//! double-counted. Numerous tiny caches (`__pycache__`, `.mypy_cache`, …) are
//! aggregated into one item per kind instead of flooding the list.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::SystemTime;

use walkdir::WalkDir;

use super::{emit, Category, ScanItem, ScanMsg, ScanOptions};
use crate::util::{dir_size, modified, newest};

struct Agg {
    category: Category,
    size: u64,
    paths: Vec<PathBuf>,
    modified: Option<SystemTime>,
}

pub fn walk_root(root: &Path, opts: &ScanOptions, tx: &Sender<ScanMsg>) {
    let mut aggregates: HashMap<&'static str, Agg> = HashMap::new();
    let mut it = WalkDir::new(root).follow_links(false).into_iter();

    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(_)) => continue, // permission denied, etc.
            Some(Ok(e)) => e,
        };
        if !entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy();

        // ── Individual items (each gets its own row) ───────────────────────
        if name == "node_modules" {
            emit_individual(tx, opts, Category::Node, "node_modules", parent_or(path), path);
            it.skip_current_dir();
            continue;
        }
        if path.join("pyvenv.cfg").is_file() {
            emit_individual(tx, opts, Category::Python, "virtualenv", path, path);
            it.skip_current_dir();
            continue;
        }
        if name == "target" && sibling_exists(path, "Cargo.toml") {
            emit_individual(tx, opts, Category::Build, "rust target", parent_or(path), path);
            it.skip_current_dir();
            continue;
        }
        if name == ".next" && sibling_exists(path, "package.json") {
            emit_individual(tx, opts, Category::Build, "next build", parent_or(path), path);
            it.skip_current_dir();
            continue;
        }

        // ── Aggregated caches (one row per kind across the whole tree) ──────
        if let Some((kind, category)) = aggregate_kind(&name) {
            let size = dir_size(path);
            let m = modified(path);
            let agg = aggregates.entry(kind).or_insert(Agg {
                category,
                size: 0,
                paths: Vec::new(),
                modified: None,
            });
            agg.size += size;
            agg.paths.push(path.to_path_buf());
            agg.modified = newest(agg.modified, m);
            it.skip_current_dir();
            continue;
        }

        // ── Noise we never want to descend into ────────────────────────────
        if matches!(
            name.as_ref(),
            ".git" | ".hg" | ".svn" | "$RECYCLE.BIN" | "System Volume Information"
        ) {
            it.skip_current_dir();
        }
    }

    // Flush the aggregated caches as one item each.
    for (kind, agg) in aggregates {
        let n = agg.paths.len();
        emit(
            tx,
            opts,
            ScanItem {
                category: agg.category,
                kind: kind.to_string(),
                label: format!("{kind} ({n} dir{})", if n == 1 { "" } else { "s" }),
                paths: agg.paths,
                size: agg.size,
                modified: agg.modified,
            },
        );
    }
}

fn aggregate_kind(name: &str) -> Option<(&'static str, Category)> {
    match name {
        "__pycache__" => Some(("__pycache__", Category::Python)),
        ".pytest_cache" => Some((".pytest_cache", Category::Python)),
        ".mypy_cache" => Some((".mypy_cache", Category::Python)),
        ".ruff_cache" => Some((".ruff_cache", Category::Python)),
        ".turbo" => Some((".turbo", Category::Build)),
        _ => None,
    }
}

fn emit_individual(
    tx: &Sender<ScanMsg>,
    opts: &ScanOptions,
    category: Category,
    kind: &str,
    label_dir: &Path,
    target: &Path,
) {
    emit(
        tx,
        opts,
        ScanItem {
            category,
            kind: kind.to_string(),
            label: dir_label(label_dir),
            paths: vec![target.to_path_buf()],
            size: dir_size(target),
            modified: modified(target),
        },
    );
}

fn parent_or(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

fn sibling_exists(path: &Path, file: &str) -> bool {
    path.parent().map_or(false, |p| p.join(file).is_file())
}

fn dir_label(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
