//! Shared project + process utilities used by both the hub and the runner:
//! detecting a project's ecosystem and launching `pnpm` / `uv` portably.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// The package ecosystem a project belongs to.
#[derive(Clone, Copy, PartialEq)]
pub enum Eco {
    Node,
    Python,
}

impl Eco {
    pub fn label(&self) -> &'static str {
        match self {
            Eco::Node => "node",
            Eco::Python => "python",
        }
    }

    /// Detect the ecosystem of `dir` from its manifest files.
    pub fn detect(dir: &Path) -> Option<Eco> {
        if dir.join("package.json").is_file() {
            Some(Eco::Node)
        } else if dir.join("pyproject.toml").is_file() || dir.join("requirements.txt").is_file() {
            Some(Eco::Python)
        } else {
            None
        }
    }
}

pub fn cwd() -> Result<PathBuf> {
    std::env::current_dir().context("cannot determine current directory")
}

/// Build a command, wrapping through `cmd /C` on Windows so that `.cmd`/`.ps1`
/// shims (npm, pnpm) resolve the same way they do in a shell.
pub fn shell_cmd(program: &str) -> Command {
    if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(program);
        c
    } else {
        Command::new(program)
    }
}

/// Is `program` available and runnable?
pub fn has_tool(program: &str) -> bool {
    shell_cmd(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Run a tool, inheriting stdio so the user sees its output live. Returns
/// whether it exited successfully.
pub fn run_tool(program: &str, args: &[&str], cwd: &Path, envs: &[(&str, &str)]) -> Result<bool> {
    println!("→ {program} {}", args.join(" "));
    let mut cmd = shell_cmd(program);
    cmd.args(args).current_dir(cwd);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .with_context(|| format!("could not launch `{program}` — is it installed and on PATH?"))?;
    Ok(status.success())
}

/// Persist a user-scoped environment variable (best-effort) so future shells
/// pick it up. On Windows this uses `setx`.
pub fn persist_env(key: &str, value: &str) {
    if cfg!(windows) {
        let _ = Command::new("cmd")
            .args(["/C", "setx", key, value])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    std::env::set_var(key, value);
}
