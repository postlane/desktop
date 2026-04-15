// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { formatTimestamp } from './TimezoneContext';

describe('formatTimestamp', () => {
  it('returns — for null', () => {
    expect(formatTimestamp(null, 'UTC')).toBe('—');
  });

  it('returns — for undefined', () => {
    expect(formatTimestamp(undefined, 'UTC')).toBe('—');
  });

  it('returns — for empty string', () => {
    expect(formatTimestamp('', 'UTC')).toBe('—');
  });

  it('formats a valid ISO timestamp with a specific timezone', () => {
    const result = formatTimestamp('2026-04-15T10:00:00Z', 'UTC');
    expect(result).toContain('2026');
  });

  it('does not throw for invalid timezone — falls back gracefully', () => {
    expect(() =>
      formatTimestamp('2026-04-15T10:00:00Z', 'Not/ATimezone'),
    ).not.toThrow();
  });

  it('uses locale default when timezone is empty string', () => {
    const result = formatTimestamp('2026-04-15T10:00:00Z', '');
    expect(result).toBeTruthy();
    expect(result).not.toBe('—');
  });
});
