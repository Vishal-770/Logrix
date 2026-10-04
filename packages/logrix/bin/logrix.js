#!/usr/bin/env node

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const targetPlatform = os.platform();
const targetArch = os.arch();

// Check if local Rust build binary exists in workspace or system PATH
let binaryPath = "logrix";
const localDebugPath = path.resolve(__dirname, "../../../target/debug/logrix");
const localReleasePath = path.resolve(__dirname, "../../../target/release/logrix");

if (fs.existsSync(localReleasePath)) {
  binaryPath = localReleasePath;
} else if (fs.existsSync(localDebugPath)) {
  binaryPath = localDebugPath;
}

const args = process.argv.slice(2);
const result = spawnSync(binaryPath, args, { stdio: "inherit" });

if (result.error) {
  if (result.error.code === "ENOENT") {
    console.error("Logrix binary not found in PATH or target directories.");
    console.error("Please run: cargo build --bin logrix");
  } else {
    console.error("Failed to execute logrix binary:", result.error.message);
  }
  process.exit(1);
}

process.exit(result.status !== null ? result.status : 0);
