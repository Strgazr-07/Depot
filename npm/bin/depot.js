#!/usr/bin/env node
"use strict";

// Thin launcher: npm installs this JS shim as the `depot` command, and the
// postinstall step (install.js) downloads the matching native binary into
// ../binaries. Here we just exec that binary, passing through args/stdio and
// the exit code.

const { spawnSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const exe = process.platform === "win32" ? "depot.exe" : "depot";
const binary = path.join(__dirname, "..", "binaries", exe);

if (!fs.existsSync(binary)) {
  console.error(
    "[depot] native binary not found.\n" +
      "        Reinstall with:  npm install -g getdepot\n" +
      "        or build from source: cargo install --git <repo>"
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  console.error("[depot] failed to launch:", result.error.message);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
