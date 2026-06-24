// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const BETA_URL =
  'https://github.com/postlane/desktop/releases/download/beta/latest.json';

const workflowPath = resolve(process.cwd(), '.github/workflows/beta-build.yml');
const exists = existsSync(workflowPath);
const content = exists ? readFileSync(workflowPath, 'utf-8') : '';

describe('beta-build.yml workflow', () => {
  it('file exists', () => {
    expect(exists).toBe(true);
  });

  it('triggers on push to beta branch only', () => {
    expect(content).toContain('push:');
    expect(content).toContain('beta');
    // Must not trigger on push to main (that belongs to the stable release workflow)
    const pushBranchesBlock = content.match(/on:\s+push:\s+branches:([\s\S]*?)(?:\n\w|\n\s*\n|$)/)?.[1] ?? '';
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

  it('uses the Tauri signing secrets', () => {
    expect(content).toContain('TAURI_SIGNING_PRIVATE_KEY');
    expect(content).toContain('TAURI_SIGNING_PRIVATE_KEY_PASSWORD');
  });

  it('all action references are SHA-pinned (no floating version tags)', () => {
    const usesLines = content
      .split('\n')
      .filter((l) => /^\s+uses:\s/.test(l));
    for (const line of usesLines) {
      // Extract just the reference part (strip inline comments and whitespace).
      const ref = line.match(/uses:\s+(\S+)/)?.[1] ?? '';
      expect(ref, `floating tag in: ${line.trim()}`).not.toMatch(/@v\d+(\.|$)/);
      expect(ref, `floating @stable in: ${line.trim()}`).not.toMatch(/@stable$/);
      expect(ref, `floating @main in: ${line.trim()}`).not.toMatch(/@main$/);
    }
  });
});
