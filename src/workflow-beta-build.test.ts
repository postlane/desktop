// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import content from '../.github/workflows/beta-build.yml?raw';

const BETA_URL =
  'https://github.com/postlane/desktop/releases/download/beta/latest.json';

describe('beta-build.yml workflow', () => {
  it('is non-empty', () => {
    expect(content.length).toBeGreaterThan(0);
  });

  it('triggers on push to beta branch only', () => {
    expect(content).toContain('push:');
    expect(content).toContain('beta');
    // Must not trigger on push to main — that belongs to the stable release workflow.
    const pushBranchesBlock =
      content.match(/on:\s+push:\s+branches:([\s\S]*?)(?:\n\w|\n\s*\n|$)/)?.[1] ?? '';
    expect(pushBranchesBlock).not.toContain('main');
  });

  it('sets TAURI_UPDATER_ENDPOINT to the beta feed URL', () => {
    expect(content).toContain('TAURI_UPDATER_ENDPOINT');
    expect(content).toContain(BETA_URL);
  });

  it('marks the GitHub release as a pre-release', () => {
    expect(content).toContain('prerelease: true');
  });

  it('body includes the required beta notice', () => {
    expect(content).toContain('Beta release');
    expect(content).toContain('not for production use');
  });

  it('specifies toolchain: stable for dtolnay/rust-toolchain', () => {
    expect(content).toContain('toolchain: stable');
  });

  it('uses the Tauri signing secrets', () => {
    expect(content).toContain('TAURI_SIGNING_PRIVATE_KEY');
    expect(content).toContain('TAURI_SIGNING_PRIVATE_KEY_PASSWORD');
  });

  // F2: darwin-x86_64 platform URL must use _x86_64 suffix, not _x64.
  // Tauri names the macOS Intel artifact Postlane_<version>_x86_64.app.tar.gz;
  // the _x64 suffix matches no file and the updater silently serves a broken URL.
  it('darwin-x86_64 latest.json URL uses _x86_64 suffix, not _x64', () => {
    expect(
      content,
      'darwin-x86_64 URL uses _x64 suffix — Tauri names the artifact _x86_64; the updater will serve a 404',
    ).not.toContain('_x64.app.tar.gz');
    expect(content).toContain('_x86_64.app.tar.gz');
  });

  // Sweep: verify the correct artifact filenames for all three platform URLs
  it('darwin-aarch64 URL uses _aarch64.app.tar.gz suffix', () => {
    expect(content).toContain('_aarch64.app.tar.gz');
  });

  it('linux-x86_64 URL uses .AppImage suffix (amd64 Tauri artifact)', () => {
    expect(content).toContain('_amd64.AppImage');
  });
});
