//! `depot run` / `depot ensure` — assemble a project's dependencies from the
//! shared global store on demand (installing only what's missing), then run.
//!
//! Node is routed through pnpm against the shared store; Python through
//! `uv run`, which already syncs the environment from uv's global cache before
//! running. The first time a package is needed anywhere it's fetched once into
//! the store; every project after that just hard-links it — so you never run a
//! manual `install` step, and only genuinely new packages hit the network.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::hub::registry::Registry;
use crate::hub::store::{ensure_store, pnpm_store, uv_cache};
use crate::project::{cwd, has_tool, run_tool, Eco};
use crate::util::modified;

/// `depot ensure`: make the project's deps present (link from store, fetch
/// missing) without running anything — handy in CI or to pre-warm.
pub fn ensure() -> Result<()> {
    let dir = cwd()?;
    let eco = detect(&dir)?;
    ensure_store()?;
    let ok = assemble(&dir, eco)?;
    register(&dir, eco)?;
    if !ok {
        bail!("dependency install reported an error");
    }
    println!("✓ {} project ready — deps linked from the shared store", eco.label());
    Ok(())
}

/// `depot run [script/args…]`: assemble deps, then run.
pub fn run(args: Vec<String>) -> Result<()> {
    let dir = cwd()?;
    let eco = detect(&dir)?;
    ensure_store()?;
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let ok = match eco {
        Eco::Node => {
            assemble(&dir, eco)?;
            register(&dir, eco)?;
            if refs.is_empty() {
                bail!("specify a script to run, e.g. `depot run dev`");
            }
            let mut a = vec!["run"];
            a.extend_from_slice(&refs);
            run_tool("pnpm", &a, &dir, &[])?
        }
        Eco::Python => {
            register(&dir, eco)?;
            // `uv run` syncs from the global cache (installing missing) then runs.
            let cache = uv_cache().to_string_lossy().to_string();
            let env = [("UV_CACHE_DIR", cache.as_str())];
            if refs.is_empty() {
                run_tool("uv", &["sync"], &dir, &env)?
            } else {
                let mut a = vec!["run"];
                a.extend_from_slice(&refs);
                run_tool("uv", &a, &dir, &env)?
            }
        }
    };

    if !ok {
        bail!("the command exited with an error");
    }
    Ok(())
}

fn detect(dir: &Path) -> Result<Eco> {
    Eco::detect(dir).context(
        "not a Node or Python project here (need package.json / pyproject.toml / requirements.txt)",
    )
}

/// Ensure the project's dependencies are present, pulling from the shared store
/// and fetching only what's missing.
fn assemble(dir: &Path, eco: Eco) -> Result<bool> {
    match eco {
        Eco::Node => assemble_node(dir),
        Eco::Python => {
            let cache = uv_cache().to_string_lossy().to_string();
            let env = [("UV_CACHE_DIR", cache.as_str())];
            if dir.join("pyproject.toml").is_file() {
                run_tool("uv", &["sync"], dir, &env)
            } else {
                if !dir.join(".venv").exists() {
                    run_tool("uv", &["venv"], dir, &env)?;
                }
                if dir.join("requirements.txt").is_file() {
                    run_tool("uv", &["pip", "install", "-r", "requirements.txt"], dir, &env)
                } else {
                    Ok(true)
                }
            }
        }
    }
}

/// Store-backed install for Node. Skips work entirely when `node_modules` is
/// already fresh (manifest/lockfiles unchanged since the last assemble), so
/// repeat runs start instantly — container-style.
fn assemble_node(dir: &Path) -> Result<bool> {
    if node_fresh(dir) {
        println!("• deps already assembled from the store (skipping install)");
        return Ok(true);
    }
    if !has_tool("pnpm") {
        bail!("pnpm is required for Node projects — `npm install -g pnpm` or run `depot hub init`");
    }
    let store_s = pnpm_store().to_string_lossy().to_string();
    let ok = run_tool("pnpm", &["install", "--store-dir", &store_s], dir, &[])?;
    if ok {
        let _ = std::fs::write(stamp_path(dir), b"depot");
    }
    Ok(ok)
}

fn stamp_path(dir: &Path) -> PathBuf {
    dir.join("node_modules").join(".depot-stamp")
}

/// `node_modules` is fresh iff it exists and every manifest/lockfile is older
/// than the stamp we wrote after the last successful assemble.
fn node_fresh(dir: &Path) -> bool {
    if !dir.join("node_modules").is_dir() {
        return false;
    }
    let Some(stamp) = modified(&stamp_path(dir)) else {
        return false;
    };
    for f in ["package.json", "pnpm-lock.yaml", "package-lock.json"] {
        if let Some(m) = modified(&dir.join(f)) {
            if m > stamp {
                return false;
            }
        }
    }
    true
}

fn register(dir: &Path, eco: Eco) -> Result<()> {
    let mut reg = Registry::load();
    reg.upsert(&dir.to_string_lossy(), eco.label());
    reg.save()
}
