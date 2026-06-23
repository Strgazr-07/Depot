# Publishing depot

`depot` is a Rust binary distributed through **npm** under the package name
**`getdepot`**. npm is just the installer — like using one browser to download
another. After `npm install -g getdepot`, users run the `depot` command, which
then manages their packages through the shared store.

```
 npm install -g getdepot   →   downloads the depot binary   →   depot run / depot hub add …
   (the bootstrap)               (postinstall, per-OS)            (the real tool)
```

## How distribution works

- **GitHub Releases** hold the prebuilt binaries (`depot-<target>[.exe]`), one
  per platform, built by [`.github/workflows/release.yml`](.github/workflows/release.yml).
- The **npm package** [`npm/`](npm/) contains only a tiny JS launcher
  ([`bin/depot.js`](npm/bin/depot.js)) and a postinstall script
  ([`install.js`](npm/install.js)). On install it downloads the binary matching
  the user's OS/arch from the release whose tag is `v<version>`.
- So **the npm version, the git tag, and the release must all match**
  (e.g. `0.2.1` ⇄ `v0.2.1`).

## One-time setup

1. **Create the GitHub repo** and push this project to it.
2. Replace `Strgazr-07` with your GitHub user/org in:
   - [`npm/package.json`](npm/package.json) → `repository`, `homepage`, `bugs`
   - [`npm/README.md`](npm/README.md) and the root [`README.md`](README.md)

   (`install.js` reads the repo from `package.json`, so that's the source of truth.)
3. **Create an npm account** and log in locally: `npm login`.
4. Confirm the name is free (it is, as of writing): `npm view getdepot` → 404.

## Cutting a release

1. **Bump the version in both places (keep them identical):**
   - `Cargo.toml` → `version`
   - `npm/package.json` → `version`
2. Commit, then tag and push the tag:
   ```bash
   git commit -am "release v0.2.1"
   git tag v0.2.1
   git push && git push --tags
   ```
3. The **release workflow** runs on the tag: it builds Windows / macOS (x64 +
   arm64) / Linux binaries and attaches them to the GitHub Release `v0.2.1`.
   Wait for it to finish (Actions tab).
4. **Publish to npm** (binaries are downloaded by users from the release, not
   bundled, so publish *after* the release assets exist):
   ```bash
   cd npm
   npm publish        # add --otp=<code> if you have 2FA enabled
   ```

## Verifying the published package

```bash
npm install -g getdepot
depot --version
depot scan
```

On install you should see `[depot] downloading depot-<target> … installed ✓`.

## Notes

- **Soft-fail install:** if a platform has no prebuilt binary, the postinstall
  warns instead of failing `npm install`; the launcher then explains how to
  build from source. To add a platform, add it to the workflow matrix **and** to
  `PLATFORM_TARGETS` in `install.js` (currently: win-x64, mac-x64, mac-arm64,
  linux-x64).
- **`--ignore-scripts`:** some users/CI disable postinstall scripts. Document
  `cargo install --git <repo>` as the fallback (the launcher already hints it).
- **Upgrade path (optional):** for a fully script-free install you can later
  switch to the esbuild-style model — publish one `@getdepot/<target>` package
  per platform as `optionalDependencies`. Not needed to ship.
- **Automating npm publish:** you can add a job to the workflow that runs
  `npm publish` with an `NPM_TOKEN` secret after the build job; kept manual here
  for a first release.
