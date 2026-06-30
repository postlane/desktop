// SPDX-License-Identifier: BUSL-1.1
// Cross-layer contract: the Rust cancel_post_impl error string must contain the
// substring that RepoPublishedView.tsx uses to filter "Cancel via dashboard".

import { describe, it, expect } from 'vitest';
import rustSrc from '../../src-tauri/src/post_cancel.rs?raw';
import tsSrc from './RepoPublishedView.tsx?raw';

describe('cancel error contract', () => {
  it('Rust post_cancel.rs error string contains the substring the TS filter depends on', () => {
    const match = rustSrc.match(/Err\("([^"]+)"\s*\.to_string\(\)\)/);
    expect(match, 'cancel_post_impl must have a literal Err("...") string').toBeTruthy();
    const errorString = (match?.[1] ?? '').toLowerCase();
    expect(
      errorString.includes('not yet available'),
      'Rust error must contain "not yet available" for the TS filter to work'
    ).toBe(true);
  });

  it('TypeScript filter checks for "not yet available" substring', () => {
    expect(tsSrc).toContain("'not yet available'");
  });
});
