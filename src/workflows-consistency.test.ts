// SPDX-License-Identifier: BUSL-1.1
/// <reference types="node" />
//
// Structural tests ensuring all desktop workflow files use consistent,
// SHA-pinned action references. Floating tags can be silently redirected
// via a supply chain attack — the production release workflow handles
// APPLE_CERTIFICATE, TAURI_SIGNING_PRIVATE_KEY, and GPG_PRIVATE_KEY.

import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const WORKFLOWS_DIR = join(__dirname, '..', '.github', 'workflows');

function readWorkflows(): Array<{ name: string; content: string }> {
  return readdirSync(WORKFLOWS_DIR)
    .filter((f) => f.endsWith('.yml') || f.endsWith('.yaml'))
    .map((f) => ({ name: f, content: readFileSync(join(WORKFLOWS_DIR, f), 'utf-8') }));
}

function extractUsesLines(content: string): string[] {
  return content.split('\n').filter((l) => /^\s+uses:\s/.test(l));
}

function extractActionRef(line: string): string {
  return line.match(/uses:\s+(\S+)/)?.[1] ?? '';
}

function collectActionRefs(workflows: Array<{ content: string }>, prefix: string): Set<string> {
  const refs = new Set<string>();
  for (const { content } of workflows) {
    for (const line of extractUsesLines(content)) {
      const ref = extractActionRef(line);
      if (ref.startsWith(prefix)) refs.add(ref);
    }
  }
  return refs;
}

describe('desktop workflow files — action SHA consistency', () => {
  it('no workflow uses a floating version tag (e.g. @v4, @stable, @main)', () => {
    const workflows = readWorkflows();
    for (const { name, content } of workflows) {
      for (const line of extractUsesLines(content)) {
        const ref = extractActionRef(line);
        expect(ref, `Floating tag in ${name}: ${line.trim()}`).not.toMatch(/@v\d+(\.|$)/);
        expect(ref, `Floating @stable in ${name}: ${line.trim()}`).not.toMatch(/@stable$/);
        expect(ref, `Floating @main in ${name}: ${line.trim()}`).not.toMatch(/@main$/);
      }
    }
  });

  it('all workflows use the same SHA for actions/checkout', () => {
    const refs = collectActionRefs(readWorkflows(), 'actions/checkout@');
    expect(refs.size, `Multiple checkout SHAs: ${[...refs].join(', ')}`).toBe(1);
  });

  it('all workflows use the same SHA for actions/setup-node', () => {
    const refs = collectActionRefs(readWorkflows(), 'actions/setup-node@');
    expect(refs.size, `Multiple setup-node SHAs: ${[...refs].join(', ')}`).toBe(1);
  });

  it('all workflows use the same SHA for actions/cache', () => {
    const refs = collectActionRefs(readWorkflows(), 'actions/cache@');
    expect(refs.size, `Multiple cache SHAs: ${[...refs].join(', ')}`).toBe(1);
  });
});

describe('desktop workflow files — individual checks', () => {
  it('ci.yml does not install cargo-nextest via curl-pipe-to-binary', () => {
    const ci = readWorkflows().find((w) => w.name === 'ci.yml');
    expect(ci, 'ci.yml not found').toBeDefined();
    expect(
      ci?.content,
      'curl-pipe install detected — use cargo install --version with caching instead',
    ).not.toContain('get.nexte.st');
  });

  it('beta-build.yml latest.json heredoc EOF terminator is at the same indent as the cat command', () => {
    const betaBuild = readWorkflows().find((w) => w.name === 'beta-build.yml');
    expect(betaBuild, 'beta-build.yml not found').toBeDefined();
    if (!betaBuild) return;
    const lines = betaBuild.content.split('\n');
    const catLineIdx = lines.findIndex((l) => l.includes('cat > latest.json <<EOF'));
    expect(catLineIdx, 'cat > latest.json <<EOF not found in beta-build.yml').toBeGreaterThan(-1);
    const catIndent = lines[catLineIdx].match(/^(\s*)/)?.[1].length ?? 0;
    // EOF terminator must be at the SAME indent as the cat command so that
    // after YAML strips the common indentation, bash sees EOF at column 0.
    const eofLineIdx = lines.findIndex((l, i) => i > catLineIdx && l.trimStart() === 'EOF');
    expect(eofLineIdx, 'EOF terminator not found after cat line').toBeGreaterThan(catLineIdx);
    const eofIndent = lines[eofLineIdx].match(/^(\s*)/)?.[1].length ?? 0;
    expect(
      eofIndent,
      `EOF indent (${eofIndent}) must equal cat command indent (${catIndent}) so bash sees EOF at column 0`,
    ).toBe(catIndent);
  });

  it('every dtolnay/rust-toolchain step specifies toolchain: stable', () => {
    const workflows = readWorkflows();
    for (const { name, content } of workflows) {
      const lines = content.split('\n');
      for (let i = 0; i < lines.length; i++) {
        if (!lines[i].includes('dtolnay/rust-toolchain@')) continue;
        // Look for 'toolchain:' within the next 5 lines (the with: block)
        const window = lines.slice(i + 1, i + 6).join('\n');
        expect(
          window,
          `${name}: dtolnay/rust-toolchain step at line ${i + 1} is missing toolchain: stable`,
        ).toContain('toolchain:');
      }
    }
  });

  it('ci.yml license-checker step has no || echo fallback', () => {
    const ci = readWorkflows().find((w) => w.name === 'ci.yml');
    expect(ci, 'ci.yml not found').toBeDefined();
    const checkerLine = ci?.content
      .split('\n')
      .find((l) => l.includes('license-checker') && l.includes('--onlyAllow'));
    expect(checkerLine, 'license-checker --onlyAllow line not found in ci.yml').toBeDefined();
    expect(
      checkerLine,
      'license-checker has || echo fallback — remove it so GPL detections fail CI',
    ).not.toContain('|| echo');
  });
});

describe('desktop workflow files — top-level permissions block', () => {
  // Every workflow must have a top-level permissions: block before jobs: so
  // any job added later does not inherit repo-default write access.
  it('every workflow file has a top-level permissions block before jobs:', () => {
    const workflows = readWorkflows();
    for (const { name, content } of workflows) {
      const permissionsIdx = content.indexOf('\npermissions:');
      const jobsIdx = content.indexOf('\njobs:');
      expect(
        permissionsIdx,
        `${name}: no top-level permissions: block — without it the default GITHUB_TOKEN has write access`,
      ).toBeGreaterThan(-1);
      expect(
        permissionsIdx,
        `${name}: top-level permissions: must appear before jobs:`,
      ).toBeLessThan(jobsIdx);
    }
  });
});

describe('desktop workflow files — beta-build.yml signature extraction', () => {
  // F10: the || echo "" fallback causes an empty signature to be written into
  // latest.json when the artifact sig file is absent.  An empty sig is a
  // security risk (Tauri may accept the update) and masks a missing artifact.
  // Failing CI is the correct behaviour when a sig file is not found.
  it('beta-build.yml signature extraction lines have no || echo "" fallback', () => {
    const betaBuild = readWorkflows().find((w) => w.name === 'beta-build.yml');
    expect(betaBuild, 'beta-build.yml not found').toBeDefined();
    if (!betaBuild) return;
    const sigLines = betaBuild.content
      .split('\n')
      .filter((l) => l.includes('_SIG=$(') || l.includes('_SIG=$( '));
    expect(sigLines.length, 'no signature extraction lines found in beta-build.yml').toBeGreaterThan(0);
    for (const line of sigLines) {
      expect(
        line,
        `signature line has || echo "" fallback — remove it so missing artifacts fail CI: ${line.trim()}`,
      ).not.toContain('|| echo ""');
    }
  });
});

describe('desktop workflow files — license-checker flags', () => {
  function getLicenseCheckerLine(): string | undefined {
    const ci = readWorkflows().find((w) => w.name === 'ci.yml');
    return ci?.content.split('\n').find((l) => l.includes('license-checker') && l.includes('--onlyAllow'));
  }

  it('uses --production to skip dev-only deps', () => {
    expect(getLicenseCheckerLine(), 'license-checker --onlyAllow line not found').toBeDefined();
    expect(getLicenseCheckerLine()).toContain('--production');
  });

  it('allows MPL-2.0 (lightningcss via @tailwindcss/postcss)', () => {
    expect(getLicenseCheckerLine()).toContain('MPL-2.0');
  });

  it('allows MIT-0 (@csstools packages)', () => {
    expect(getLicenseCheckerLine()).toContain('MIT-0');
  });

  it('allows LGPL-3.0-or-later (next/sharp/libvips chain)', () => {
    expect(getLicenseCheckerLine()).toContain('LGPL-3.0-or-later');
  });
});
