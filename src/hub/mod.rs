//! Feature 4 — the **hub**: one set of commands that routes both Node (pnpm)
//! and Python (uv) installs through a single shared, content-addressable global
//! store. The standout idea is a *cross-ecosystem* package store: `depot hub
//! add <pkg>` works the same whether you're in a Node or a Python project,
//! every project is registered centrally, and packages are stored once on disk
//! and hard-linked into each project instead of re-downloaded.

pub mod registry;
pub mod store;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::cli::HubAction;
use crate::project::{cwd, has_tool, persist_env, run_tool, Eco};
use crate::scan::{Category, ScanOptions};
use crate::util::{dir_size, human_size};
use registry::Registry;
use store::{ensure_store, pnpm_store, store_root, uv_cache, HubConfig};

pub fn run(action: HubAction) -> Result<()> {
    match action {
        HubAction::Init => init(),
        HubAction::Add { packages, dev } => add(packages, dev),
        HubAction::Status => status(),
        HubAction::Link => link(),
        HubAction::Analyze { paths } => analyze(paths),
    }
}

// ── Commands ────────────────────────────────────────────────────────────────

fn init() -> Result<()> {
    println!("Setting up the shared store under {}\n", store_root().display());
    let mut cfg = ensure_store()?;

    // pnpm — install via corepack if missing, then point its store at ours.
    if !has_tool("pnpm") {
        println!("pnpm not found — trying to enable it via corepack…");
        let _ = run_tool("corepack", &["enable"], &cwd()?, &[]);
        let _ = run_tool("corepack", &["prepare", "pnpm@latest", "--activate"], &cwd()?, &[]);
    }
    if has_tool("pnpm") {
        let store = pnpm_store();
        let store_s = store.to_string_lossy().to_string();
        let ok = run_tool(
            "pnpm",
            &["config", "set", "store-dir", &store_s, "--global"],
            &cwd()?,
            &[],
        )?;
        println!(
            "{} pnpm store-dir → {}",
            if ok { "✓" } else { "!" },
            store.display()
        );
    } else {
        println!("! pnpm still unavailable; install Node + corepack, then re-run `hub init`.");
    }

    // uv — already cache-based; just unify the cache location.
    if has_tool("uv") {
        persist_env("UV_CACHE_DIR", &uv_cache().to_string_lossy());
        println!("✓ uv cache → {}", uv_cache().display());
    } else {
        println!("! uv not found; install it (https://docs.astral.sh/uv) to use Python.");
    }

    cfg.initialized = true;
    cfg.save()?;
    println!("\nDone. Use `depot hub add <pkg>` or `depot run <script>` in any project.");
    Ok(())
}

fn add(packages: Vec<String>, dev: bool) -> Result<()> {
    let dir = cwd()?;
    let eco = Eco::detect(&dir).context(
        "no package.json / pyproject.toml / requirements.txt here — run this inside a project",
    )?;
    ensure_store()?;
    let before = store_size();

    let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
    let ok = match eco {
        Eco::Node => {
            let store_s = pnpm_store().to_string_lossy().to_string();
            let mut args = vec!["add"];
            args.extend_from_slice(&pkg_refs);
            if dev {
                args.push("-D");
            }
            args.push("--store-dir");
            args.push(&store_s);
            run_tool("pnpm", &args, &dir, &[])?
        }
        Eco::Python => add_python(&dir, &pkg_refs, dev)?,
    };

    if !ok {
        bail!("the package manager reported an error");
    }

    Registry::load_then(|r| r.upsert(&dir.to_string_lossy(), eco.label()))?;

    let delta = store_size().saturating_sub(before);
    println!(
        "\n✓ added {} to {} project · store grew by {} (shared across all projects)",
        packages.join(", "),
        eco.label(),
        human_size(delta),
    );
    Ok(())
}

fn add_python(dir: &Path, pkgs: &[&str], dev: bool) -> Result<bool> {
    let cache = uv_cache().to_string_lossy().to_string();
    let env = [("UV_CACHE_DIR", cache.as_str())];

    if dir.join("pyproject.toml").is_file() {
        let mut args = vec!["add"];
        args.extend_from_slice(pkgs);
        if dev {
            args.push("--dev");
        }
        run_tool("uv", &args, dir, &env)
    } else {
        // requirements.txt-only project: maintain a uv-managed .venv.
        if !dir.join(".venv").exists() {
            run_tool("uv", &["venv"], dir, &env)?;
        }
        let mut args = vec!["pip", "install"];
        args.extend_from_slice(pkgs);
        run_tool("uv", &args, dir, &env)
    }
}

fn status() -> Result<()> {
    let cfg = HubConfig::load();
    let reg = Registry::load();

    println!("depot hub — shared package store\n");
    println!("  store root : {}", store_root().display());
    let _ = cfg.initialized;
    println!(
        "  pnpm store : {:>10}  ({})",
        human_size(dir_size(&pnpm_store())),
        if has_tool("pnpm") { "ready" } else { "pnpm missing" },
    );
    println!(
        "  uv cache   : {:>10}  ({})",
        human_size(dir_size(&uv_cache())),
        if has_tool("uv") { "ready" } else { "uv missing" },
    );
    println!("  total      : {:>10}", human_size(store_size()));

    println!("\n  linked projects: {}", reg.projects.len());
    for p in &reg.projects {
        println!("    [{:<6}] {}", p.ecosystem, p.path);
    }
    if reg.projects.len() > 1 {
        println!(
            "\n  {} projects share one store — every common package is stored once, not per-project.",
            reg.projects.len()
        );
    }
    Ok(())
}

fn link() -> Result<()> {
    let dir = cwd()?;
    let eco = Eco::detect(&dir).context("not a Node or Python project")?;
    ensure_store()?;

    let cache = uv_cache().to_string_lossy().to_string();
    let store_s = pnpm_store().to_string_lossy().to_string();
    let ok = match eco {
        Eco::Node => run_tool("pnpm", &["install", "--store-dir", &store_s], &dir, &[])?,
        Eco::Python => {
            let env = [("UV_CACHE_DIR", cache.as_str())];
            if dir.join("pyproject.toml").is_file() {
                run_tool("uv", &["sync"], &dir, &env)?
            } else if dir.join("requirements.txt").is_file() {
                run_tool("uv", &["venv"], &dir, &env)?;
                run_tool("uv", &["pip", "install", "-r", "requirements.txt"], &dir, &env)?
            } else {
                false
            }
        }
    };

    Registry::load_then(|r| r.upsert(&dir.to_string_lossy(), eco.label()))?;
    println!(
        "\n{} linked {} project to the shared store.",
        if ok { "✓" } else { "!" },
        eco.label()
    );
    Ok(())
}

fn analyze(paths: Vec<PathBuf>) -> Result<()> {
    let roots = if paths.is_empty() {
        vec![crate::scan::default_root()]
    } else {
        paths
    };
    println!("Scanning for projects under {} root(s)…", roots.len());

    let opts = ScanOptions {
        roots,
        categories: Some([Category::Node, Category::Python].into_iter().collect()),
        min_size: 0,
    };
    let items = crate::report::collect(opts);

    let node: Vec<_> = items.iter().filter(|i| i.kind == "node_modules").collect();
    let venv: Vec<_> = items.iter().filter(|i| i.kind == "virtualenv").collect();
    let node_total: u64 = node.iter().map(|i| i.size).sum();
    let venv_total: u64 = venv.iter().map(|i| i.size).sum();

    println!("\n  node_modules : {:>3} projects · {}", node.len(), human_size(node_total));
    println!("  virtualenvs  : {:>3} projects · {}", venv.len(), human_size(venv_total));
    println!("  currently using: {}", human_size(node_total + venv_total));
    println!("  shared store now: {}", human_size(store_size()));
    println!(
        "\n  These projects each keep a private copy of their dependencies. Run `depot link`\n  in each (or `depot run` / `depot hub add` going forward) to store every package once\n  and hard-link it in — duplicates across projects stop costing extra disk."
    );
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn store_size() -> u64 {
    dir_size(&pnpm_store()) + dir_size(&uv_cache())
}

impl Registry {
    /// Load, mutate, and save in one shot.
    fn load_then(f: impl FnOnce(&mut Registry)) -> Result<()> {
        let mut reg = Registry::load();
        f(&mut reg);
        reg.save()
    }
}
