// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { PROVIDERS } from './SchedulerTab';

describe('SchedulerTab — PROVIDERS (§review-critical)', () => {
  it('does not include "buffer" (no public API available)', () => {
    expect(PROVIDERS).not.toContain('buffer');
  });

  it('does not include "ayrshare" ($129/month, not viable for v1 target audience)', () => {
    expect(PROVIDERS).not.toContain('ayrshare');
  });

  it('always includes "zernio" (the only viable v1 provider)', () => {
    expect(PROVIDERS).toContain('zernio');
  });
});
