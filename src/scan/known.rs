//! Probes of fixed, well-known locations: package-manager caches, ML model
//! stores, and conda environments. These don't require walking the tree.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use super::{emit, Category, ScanItem, ScanMsg, ScanOptions};
use crate::util::{dir_size, modified};

pub fn scan_known(opts: &ScanOptions, tx: &Sender<ScanMsg>) {
    let Some(home) = dirs::home_dir() else {
        return;
    };

    for (path, kind) in node_caches(&home) {
        emit_dir(tx, opts, Category::Node, kind, path);
    }
    for (path, kind) in python_caches(&home) {
        emit_dir(tx, opts, Category::Python, kind, path);
    }

    huggingface_models(&home, opts, tx);
    emit_dir(tx, opts, Category::Model, "hf datasets", hf_root(&home).join("datasets"));
    emit_dir(tx, opts, Category::Model, "torch hub", home.join(".cache").join("torch"));
    emit_dir(tx, opts, Category::Model, "ollama models", home.join(".ollama").join("models"));
    emit_dir(tx, opts, Category::Model, "keras models", home.join(".keras").join("models"));

    conda_envs(&home, opts, tx);
}

/// Size `path`; emit a single-row item if it exists and is non-empty.
fn emit_dir(tx: &Sender<ScanMsg>, opts: &ScanOptions, category: Category, kind: &str, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    emit(
        tx,
        opts,
        ScanItem {
            category,
            kind: kind.to_string(),
            label: kind.to_string(),
            size: dir_size(&path),
            modified: modified(&path),
            paths: vec![path],
        },
    );
}

#[cfg(windows)]
fn node_caches(home: &Path) -> Vec<(PathBuf, &'static str)> {
    let local = dirs::cache_dir().unwrap_or_else(|| home.join("AppData").join("Local"));
    vec![
        (local.join("npm-cache"), "npm cache"),
        (local.join("Yarn").join("Cache"), "yarn cache"),
        (local.join("pnpm").join("store"), "pnpm store"),
        (home.join(".bun").join("install").join("cache"), "bun cache"),
    ]
}

#[cfg(not(windows))]
fn node_caches(home: &Path) -> Vec<(PathBuf, &'static str)> {
    vec![
        (home.join(".npm").join("_cacache"), "npm cache"),
        (home.join(".cache").join("yarn"), "yarn cache"),
        (home.join(".local").join("share").join("pnpm").join("store"), "pnpm store"),
        (home.join(".bun").join("install").join("cache"), "bun cache"),
    ]
}

#[cfg(windows)]
fn python_caches(home: &Path) -> Vec<(PathBuf, &'static str)> {
    let local = dirs::cache_dir().unwrap_or_else(|| home.join("AppData").join("Local"));
    vec![
        (local.join("pip").join("Cache"), "pip cache"),
        (local.join("uv").join("cache"), "uv cache"),
        (home.join(".cache").join("uv"), "uv cache"),
    ]
}

#[cfg(not(windows))]
fn python_caches(home: &Path) -> Vec<(PathBuf, &'static str)> {
    vec![
        (home.join(".cache").join("pip"), "pip cache"),
        (home.join(".cache").join("uv"), "uv cache"),
    ]
}

fn hf_root(home: &Path) -> PathBuf {
    std::env::var_os("HF_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache").join("huggingface"))
}

/// Enumerate individual models in the HuggingFace hub cache so each can be
/// selected and removed on its own (they are frequently several GB each).
fn huggingface_models(home: &Path, opts: &ScanOptions, tx: &Sender<ScanMsg>) {
    let hub = hf_root(home).join("hub");
    let Ok(read) = std::fs::read_dir(&hub) else {
        return;
    };
    for entry in read.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        if !raw.starts_with("models--") && !raw.starts_with("datasets--") {
            continue;
        }
        // `models--org--name` → `org/name`
        let label = raw
            .strip_prefix("models--")
            .or_else(|| raw.strip_prefix("datasets--"))
            .map(|rest| rest.replace("--", "/"))
            .unwrap_or_else(|| raw.clone());
        emit(
            tx,
            opts,
            ScanItem {
                category: Category::Model,
                kind: "huggingface".to_string(),
                label,
                size: dir_size(&p),
                modified: modified(&p),
                paths: vec![p],
            },
        );
    }
}

/// Each conda environment under a known conda root becomes its own row.
fn conda_envs(home: &Path, opts: &ScanOptions, tx: &Sender<ScanMsg>) {
    let roots = [
        home.join("anaconda3"),
        home.join("miniconda3"),
        home.join("miniforge3"),
        home.join(".conda"),
    ];
    for root in roots {
        let envs = root.join("envs");
        let Ok(read) = std::fs::read_dir(&envs) else {
            continue;
        };
        for entry in read.flatten() {
            let p = entry.path();
            // A real env contains a `conda-meta` directory.
            if !p.join("conda-meta").is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            emit(
                tx,
                opts,
                ScanItem {
                    category: Category::Python,
                    kind: "conda env".to_string(),
                    label: name,
                    size: dir_size(&p),
                    modified: modified(&p),
                    paths: vec![p],
                },
            );
        }
    }
}
