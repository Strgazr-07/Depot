//! A global record of every project that has been linked to the shared store.
//! Powers `hub status` and cross-project analytics.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::store::registry_path;
use crate::util::unix_secs;

#[derive(Serialize, Deserialize, Clone)]
pub struct Project {
    pub path: String,
    pub ecosystem: String,
    pub linked_at: u64,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Registry {
    pub projects: Vec<Project>,
}

impl Registry {
    pub fn load() -> Registry {
        std::fs::read_to_string(registry_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = registry_path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self).context("serializing registry")?;
        std::fs::write(registry_path(), body).context("writing registry.json")?;
        Ok(())
    }

    /// Record (or refresh) a project. De-duplicates by path.
    pub fn upsert(&mut self, path: &str, ecosystem: &str) {
        let now = unix_secs(std::time::SystemTime::now()).unwrap_or(0);
        if let Some(existing) = self.projects.iter_mut().find(|p| p.path == path) {
            existing.ecosystem = ecosystem.to_string();
            existing.linked_at = now;
        } else {
            self.projects.push(Project {
                path: path.to_string(),
                ecosystem: ecosystem.to_string(),
                linked_at: now,
            });
        }
    }
}
