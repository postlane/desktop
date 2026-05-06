// SPDX-License-Identifier: BUSL-1.1
// @vitest-environment node

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dir = dirname(fileURLToPath(import.meta.url));

function readCsp(): string {
  const raw = readFileSync(join(__dir, '../src-tauri/tauri.conf.json'), 'utf8');
  const conf = JSON.parse(raw) as { app: { security: { csp: string } } };
  return conf.app.security.csp;
}

function scriptSrc(csp: string): string {
  return csp.match(/script-src\s+([^;]+)/)?.[1] ?? '';
}

describe('tauri.conf.json — CSP security', () => {
  it("script-src must not contain 'unsafe-eval'", () => {
    expect(scriptSrc(readCsp())).not.toContain('unsafe-eval');
  });

  it("script-src must not contain 'unsafe-inline'", () => {
    expect(scriptSrc(readCsp())).not.toContain('unsafe-inline');
  });
});
