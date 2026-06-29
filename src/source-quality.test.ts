// SPDX-License-Identifier: BUSL-1.1
//
// Structural tests for source file quality standards.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ROOT = join(__dirname, '..');

function read(rel: string) {
  return readFileSync(join(ROOT, rel), 'utf-8');
}

describe('src-tauri/src/lib.rs — ENDPOINT wiring', () => {
  // F5: updater_endpoint::ENDPOINT is compiled in at build time but never consumed
  // by production app code.  Wiring it into the startup log makes the dependency
  // explicit and ensures the compile-time value appears in diagnostics.
  it('lib.rs references updater_endpoint::ENDPOINT so it is not dead code', () => {
    const content = read('src-tauri/src/lib.rs');
    expect(
      content,
      'lib.rs does not reference updater_endpoint::ENDPOINT — the constant is dead code; ' +
        'add a startup log so the baked-in endpoint is visible in diagnostics',
    ).toContain('updater_endpoint::ENDPOINT');
  });
});

describe('src-tauri/build.rs — TAURI_CONFIG mechanism', () => {
  // F1: cargo:rustc-env sets a variable available during the rustc compilation
  // phase (env!() macros) but NOT in the build-script process itself.
  // tauri_build::build() reads TAURI_CONFIG via std::env::var() from the current
  // process environment.  Using cargo:rustc-env=TAURI_CONFIG means the value is
  // never visible to tauri_build::build() so the updater endpoint override is a
  // no-op.  The fix is std::env::set_var() before the tauri_build::build() call.
  it('sets TAURI_CONFIG via std::env::set_var, not cargo:rustc-env', () => {
    const content = read('src-tauri/build.rs');
    expect(
      content,
      'build.rs uses cargo:rustc-env=TAURI_CONFIG — tauri_build::build() reads ' +
        'TAURI_CONFIG from the build-script process environment (std::env::var), ' +
        'not from cargo directives; use std::env::set_var instead',
    ).not.toContain('cargo:rustc-env=TAURI_CONFIG');
    expect(
      content,
      'build.rs must call std::env::set_var("TAURI_CONFIG", ...) so tauri_build::build() can read it',
    ).toContain('std::env::set_var');
  });
});
