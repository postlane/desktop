// SPDX-License-Identifier: BUSL-1.1
// Cross-layer contract: the Rust cancel_post_impl error string must contain the
// substring that RepoPublishedView.tsx uses to filter "Cancel via dashboard".

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const RUST_CANCEL = join(__dirname, '../../src-tauri/src/post_cancel.rs');
const TS_FILTER = join(__dirname, './RepoPublishedView.tsx');

describe('cancel error contract', () => {
  it('Rust post_cancel.rs error string contains the substring the TS filter depends on', () => {
    const rustSrc = readFileSync(RUST_CANCEL, 'utf-8');
    // Extract the Err(...) string from cancel_post_impl
    const match = rustSrc.match(/Err\("([^"]+)"\s*\.to_string\(\)\)/);
    expect(match, 'cancel_post_impl must have a literal Err("...") string').toBeTruthy();
    const errorString = (match?.[1] ?? '').toLowerCase();
    expect(
      errorString.includes('not yet available'),
      'Rust error must contain "not yet available" for the TS filter to work'
    ).toBe(true);
  });

  it('TypeScript filter checks for "not yet available" substring', () => {
    const tsSrc = readFileSync(TS_FILTER, 'utf-8');
    expect(tsSrc).toContain("'not yet available'");
  });
});
