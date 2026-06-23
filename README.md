<div align="center">

# depot

**Reclaim developer disk space — and share one global package store across all your projects.**

[![npm](https://img.shields.io/npm/v/getdepot?color=cb3837&logo=npm)](https://www.npmjs.com/package/getdepot)
[![release](https://img.shields.io/github/actions/workflow/status/YOUR_GITHUB_USERNAME/depot/release.yml?label=build)](https://github.com/YOUR_GITHUB_USERNAME/depot/actions)
[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

</div>

`depot` is a single fast binary (written in Rust) that does three things:

1. **🧹 Scan & clean** — finds the stuff quietly eating gigabytes (`node_modules`,
   Python venvs, package-manager caches, downloaded ML models, build artifacts),
   shows it grouped by category, and sends what you pick to the **Recycle Bin**.
2. **🚀 Run without installing** — `depot run dev` assembles a project's
   dependencies from a global store on demand (fetching only what's missing),
   then runs your script. No `npm install` step; repeat runs start instantly.
3. **📦 One shared store** — both **pnpm** (Node) and **uv** (Python) install
   into a single content-addressable store, so two projects never download the
   same package twice. One command, either ecosystem.

---

## Install

You install `depot` **through npm** — npm is just the bootstrap, the way you'd
use one browser to download another. After that you use the `depot` command:

```bash
npm install -g getdepot
depot --help
```

This downloads the prebuilt native binary for your OS; no Rust toolchain needed.

> **Runtime tools:** `depot run` and `depot hub` drive **pnpm** (Node) and **uv**
> (Python) under the hood. `depot hub init` will set pnpm up for you; install
> [uv](https://docs.astral.sh/uv/) for the Python side.

---

## Use it

### 🧹 Clean up disk

```bash
depot                    # interactive TUI (default)
depot scan               # headless, grouped table
depot scan --category model --min-mb 100   # only ML models over 100 MB
depot scan --json        # machine-readable
```

Detects, grouped into **Node · Python · Models · Build**:

| Group | Items |
| --- | --- |
| Node | `node_modules`; npm / yarn / pnpm / bun caches |
| Python | virtualenvs, conda envs; pip / uv caches; aggregated `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache` |
| Models | HuggingFace hub (per model) & datasets, PyTorch hub, Ollama, Keras |
| Build | Rust `target/`, Next.js `.next/`, Turbo `.turbo/` |

TUI keys: `↑/↓` move · `space` tick · `a`/`n` all/none · `1-4` filter category ·
`d` delete → Recycle Bin · `r` rescan · `q` quit.

### 🚀 Run a project without installing

```bash
depot run dev            # Node:   ensure deps from store → pnpm run dev
depot run app.py         # Python: uv run app.py (syncs from the cache first)
depot ensure             # just assemble deps (CI / pre-warm), don't run
```

The first time a package is needed *anywhere*, it's fetched once into the store;
every project after that hard-links it. Node runs skip the install entirely when
nothing changed, so `depot run` is basically instant.

### 📦 Manage the shared store

```bash
depot hub init           # create the store, point pnpm + uv at it
depot hub add react      # add a dep via the shared store (Node or Python)
depot hub status         # store size + every linked project
depot hub analyze        # how much your projects could consolidate
```

---

## How it works

Everything lives under `~/.depot/store`. pnpm and uv are already
content-addressable — they hard-link packages from a store instead of copying —
so `depot` points both at one shared location, gives them a single front door
(`add` / `run` / `link`), and keeps a global registry of linked projects. The
deduplication is real on disk; `depot` is the orchestration and UX layer on top.

---

## Build from source

Requires the Rust toolchain (`cargo install --path .` puts `depot` on your PATH).

```bash
cargo build --release    # → target/release/depot
```

<details>
<summary>Windows GNU toolchain notes</summary>

This repo targets `x86_64-pc-windows-gnu` (no Visual Studio needed):

1. `rustup-init.exe -y --default-host x86_64-pc-windows-gnu --default-toolchain stable --profile minimal`
2. `winget install -e --id BrechtSanders.WinLibs.POSIX.MSVCRT` — a full mingw-w64
   (modern `windows-sys` uses `raw-dylib`, which needs mingw's `dlltool`/`as`);
   put its `…\mingw64\bin` on `PATH`.

[`.cargo/config.toml`](.cargo/config.toml) sets `+crt-static`, so the binary is
self-contained.
</details>

---

## Publishing

Releases are built per-platform by CI and shipped via npm as `getdepot`. See
**[PUBLISHING.md](PUBLISHING.md)** for the full flow (tag → CI builds binaries →
`npm publish`).

## Roadmap

- [ ] delete-confirmation dialog + dry-run preview in the TUI
- [ ] yarn / npm-project support in `run` and `hub` (currently pnpm for Node)
- [ ] `hub doctor` to detect/repair store config
- [ ] richer `analyze` (real cross-project package-overlap estimate)
- [ ] aarch64-linux prebuilt binary

## Contributing

Issues and PRs welcome. Run `cargo build` and `cargo test`; keep changes scoped
and match the surrounding style.

## License

[MIT](LICENSE) © Shrijesh SP
