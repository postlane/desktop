// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import conf from '../src-tauri/tauri.conf.json';

const csp: string = (conf as { app: { security: { csp: string } } }).app.security.csp;

function scriptSrc(c: string): string {
  return c.match(/script-src\s+([^;]+)/)?.[1] ?? '';
}

describe('tauri.conf.json — CSP security', () => {
  it("script-src must not contain 'unsafe-eval'", () => {
    expect(scriptSrc(csp)).not.toContain('unsafe-eval');
  });

  it("script-src must not contain 'unsafe-inline'", () => {
    expect(scriptSrc(csp)).not.toContain('unsafe-inline');
  });
});
