#!/usr/bin/env bash
# dev-signed.sh — run tauri dev with automatic re-signing of the debug binary.
#
# WHY THIS EXISTS
# ---------------
# macOS evaluates a process's code identity at exec() time. In dev mode, every
# Rust rebuild produces a new ad-hoc-signed binary. macOS treats each new binary
# as a different app and re-challenges keychain access, forcing a 25-character
# password prompt every few minutes.
#
# Developer ID signing gives the binary a stable, certificate-anchored identity.
# When the user clicks "Always Allow" once, macOS stores that permission against
# the certificate (not the binary hash). Any future binary signed with the same
# Developer ID inherits that permission — no more prompts.
#
# HOW IT WORKS
# ------------
# 1. Pre-signs the binary if it already exists from a prior build.
# 2. Starts `npm run tauri -- dev` in the background.
# 3. Polls every 500ms. When the binary's mtime changes (Rust rebuilt), signs
#    it immediately. Rust incremental builds typically take 5-15 seconds;
#    Tauri waits for the build before relaunching, so we reliably sign before
#    the new process is exec'd.
#
# FIRST RUN
# ---------
# You will still get one prompt the first time (the old keychain entry was
# created by an ad-hoc build). Click "Always Allow" — that's the last time.

set -euo pipefail

BINARY="src-tauri/target/debug/postlane-desktop"
IDENTITY="Developer ID Application: Hugo Elliott (RNUCP3LV48)"
POLL_INTERVAL=0.5

_last_mtime=""

sign_binary() {
  codesign -s "$IDENTITY" -f --deep --options runtime "$BINARY" 2>/dev/null \
    && echo "[dev:signed] signed debug binary" \
    || echo "[dev:signed] codesign failed — is your Developer ID cert in the login keychain?"
  _last_mtime=$(stat -f "%m" "$BINARY" 2>/dev/null || echo "")
}

sign_if_changed() {
  if [ ! -f "$BINARY" ]; then return; fi
  local mtime
  mtime=$(stat -f "%m" "$BINARY" 2>/dev/null || echo "")
  if [ "$mtime" != "$_last_mtime" ]; then
    sign_binary
  fi
}

# Pre-sign any existing binary so the very first launch is already signed.
if [ -f "$BINARY" ]; then
  echo "[dev:signed] pre-signing existing binary..."
  sign_binary
fi

# Launch tauri dev in the background.
npm run tauri -- dev &
TAURI_PID=$!

cleanup() {
  kill "$TAURI_PID" 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Watch for rebuilds and re-sign promptly.
while kill -0 "$TAURI_PID" 2>/dev/null; do
  sign_if_changed
  sleep "$POLL_INTERVAL"
done
