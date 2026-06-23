# getdepot

Installs the **`depot`** CLI — a developer disk-space reclaimer and a shared
pnpm + uv global package store.

```bash
npm install -g getdepot
depot --help
```

`npm` is only the installer (think of it like using one browser to download
another). Once installed, you use `depot`:

```bash
depot scan          # find & clean node_modules / venvs / caches / ML models
depot run dev       # run a project, assembling deps from the shared store
depot hub add react # add a dependency via the shared store
```

Full docs & source: https://github.com/Strgazr-07/Depot

> Needs Node ≥ 16 to install. `depot run`/`hub` use **pnpm** (Node) and **uv**
> (Python) under the hood; `depot hub init` will set pnpm up for you.

MIT © Shrijesh SP
