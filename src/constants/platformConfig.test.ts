// SPDX-License-Identifier: BUSL-1.1

import { describe, expect, it } from 'vitest';
import { CHAR_LIMITS } from './platformConfig';

describe('CHAR_LIMITS', () => {
  it('contains all nine canonical platforms', () => {
    expect(CHAR_LIMITS['x']).toBe(280);
    expect(CHAR_LIMITS['bluesky']).toBe(300);
    expect(CHAR_LIMITS['mastodon']).toBe(500);
    expect(CHAR_LIMITS['linkedin']).toBe(3000);
    expect(CHAR_LIMITS['substack_notes']).toBe(280);
    expect(CHAR_LIMITS['substack']).toBe(0);
    expect(CHAR_LIMITS['product_hunt']).toBe(260);
    expect(CHAR_LIMITS['show_hn']).toBe(0);
    expect(CHAR_LIMITS['changelog']).toBe(0);
  });
});
