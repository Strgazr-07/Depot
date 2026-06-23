#!/usr/bin/env node
"use strict";

// postinstall: download the correct prebuilt `depot` binary for this platform
// from the project's GitHub Releases (tag must match this package's version),
// and place it in ./binaries so bin/depot.js can exec it.
//
// Failures are soft (exit 0) so `npm install` never hard-crashes — the launcher
// prints a clear message if the binary is missing.

const fs = require("fs");
const path = require("path");
const https = require("https");

const pkg = require("./package.json");

// `${process.platform} ${process.arch}` -> Rust target triple built by CI.
const PLATFORM_TARGETS = {
  "win32 x64": "x86_64-pc-windows-msvc",
  "darwin x64": "x86_64-apple-darwin",
  "darwin arm64": "aarch64-apple-darwin",
  "linux x64": "x86_64-unknown-linux-gnu",
};

function repoSlug() {
  const url = (pkg.repository && (pkg.repository.url || pkg.repository)) || "";
  const m = url.match(/github\.com[:/]+([^/]+)\/([^/.]+)/);
  if (!m) throw new Error('set "repository" to your GitHub repo in package.json');
  return `${m[1]}/${m[2]}`;
}

function download(url, dest, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "getdepot-installer" } }, (res) => {
        if ([301, 302, 307, 308].includes(res.statusCode) && res.headers.location) {
          res.resume();
          if (redirects > 10) return reject(new Error("too many redirects"));
          return resolve(download(res.headers.location, dest, redirects + 1));
        }
        if (res.statusCode !== 200) {
          res.resume();
          return reject(new Error(`HTTP ${res.statusCode} for ${url}`));
        }
        const file = fs.createWriteStream(dest);
        res.pipe(file);
        file.on("finish", () => file.close(() => resolve()));
        file.on("error", reject);
      })
      .on("error", reject);
  });
}

async function main() {
  const key = `${process.platform} ${process.arch}`;
  const target = PLATFORM_TARGETS[key];
  const slug = repoSlug();

  if (!target) {
    console.warn(
      `[depot] no prebuilt binary for ${key}. Build from source:\n` +
        `        cargo install --git https://github.com/${slug}`
    );
    return; // soft-exit
  }

  const isWin = process.platform === "win32";
  const asset = `depot-${target}${isWin ? ".exe" : ""}`;
  const url = `https://github.com/${slug}/releases/download/v${pkg.version}/${asset}`;

  const dir = path.join(__dirname, "binaries");
  fs.mkdirSync(dir, { recursive: true });
  const out = path.join(dir, isWin ? "depot.exe" : "depot");

  console.log(`[depot] downloading ${asset} …`);
  try {
    await download(url, out);
    if (!isWin) fs.chmodSync(out, 0o755);
    console.log("[depot] installed ✓  — run `depot --help`");
  } catch (e) {
    console.warn(`[depot] could not download the binary: ${e.message}`);
    console.warn(
      `[depot] grab it manually from https://github.com/${slug}/releases/tag/v${pkg.version}`
    );
  }
}

main().catch((e) => {
  console.warn(`[depot] install skipped: ${e.message}`);
});
