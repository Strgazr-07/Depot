//! Locations and persisted config for the shared global store.
//!
//! Everything lives under `~/.depot`:
//!   * `store/pnpm` — pnpm's content-addressable store (hard-links into projects)
//!   * `store/uv`   — uv's global cache (hard-links into venvs)
//!   * `config.toml`, `registry.json`

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".depot")
}

pub fn store_root() -> PathBuf {
    config_dir().join("store")
}

pub fn pnpm_store() -> PathBuf {
    store_root().join("pnpm")
}

pub fn uv_cache() -> PathBuf {
    store_root().join("uv")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn registry_path() -> PathBuf {
    config_dir().join("registry.json")
}

/// Persisted hub settings (mostly a record of where the store points).
#[derive(Serialize, Deserialize, Default)]
pub struct HubConfig {
    pub initialized: bool,
    pub pnpm_store: String,
    pub uv_cache: String,
}

impl HubConfig {
    pub fn load() -> HubConfig {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(config_dir())?;
        let body = toml::to_string_pretty(self).context("serializing hub config")?;
        std::fs::write(config_path(), body).context("writing config.toml")?;
        Ok(())
    }
}

/// Ensure the store directories exist; return a config reflecting them.
pub fn ensure_store() -> Result<HubConfig> {
    std::fs::create_dir_all(pnpm_store()).context("creating pnpm store")?;
    std::fs::create_dir_all(uv_cache()).context("creating uv cache")?;
    let mut cfg = HubConfig::load();
    cfg.pnpm_store = pnpm_store().display().to_string();
    cfg.uv_cache = uv_cache().display().to_string();
    Ok(cfg)
}
