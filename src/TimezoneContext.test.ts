// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { formatTimestamp, getTimezoneOffsetLabel, utcIsoToDatetimeLocal, localDatetimeToUtcIso } from './TimezoneContext';

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

describe('getTimezoneOffsetLabel', () => {
  it('returns empty string for empty timezone', () => {
    expect(getTimezoneOffsetLabel('')).toBe('');
  });

  it('returns a non-empty GMT string for UTC', () => {
    expect(getTimezoneOffsetLabel('UTC')).toMatch(/^GMT/);
  });

  it('returns a non-empty string for America/New_York', () => {
    expect(getTimezoneOffsetLabel('America/New_York')).toMatch(/^GMT[+-]/);
  });

  it('returns empty string for an invalid timezone', () => {
    expect(getTimezoneOffsetLabel('Not/Valid/Zone')).toBe('');
  });
});

describe('utcIsoToDatetimeLocal', () => {
  it('returns empty string for empty input', () => {
    expect(utcIsoToDatetimeLocal('', 'UTC')).toBe('');
  });

  it('converts UTC ISO to datetime-local format in UTC', () => {
    const result = utcIsoToDatetimeLocal('2026-06-01T10:00:00Z', 'UTC');
    expect(result).toBe('2026-06-01T10:00');
  });

  it('shifts to timezone offset for Europe/London BST (UTC+1)', () => {
    const result = utcIsoToDatetimeLocal('2026-06-01T08:30:00Z', 'Europe/London');
    expect(result).toBe('2026-06-01T09:30');
  });

  it('produces a string in YYYY-MM-DDTHH:mm format', () => {
    const result = utcIsoToDatetimeLocal('2026-06-01T10:00:00Z', 'UTC');
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/);
  });
});

describe('localDatetimeToUtcIso', () => {
  it('converts datetime-local in UTC to ISO UTC string', () => {
    const result = localDatetimeToUtcIso('2026-06-01T14:00', 'UTC');
    expect(result).toBe('2026-06-01T14:00:00.000Z');
  });

  it('converts datetime-local in Europe/London BST to UTC', () => {
    const result = localDatetimeToUtcIso('2026-06-01T09:30', 'Europe/London');
    expect(result).toBe('2026-06-01T08:30:00.000Z');
  });

  it('falls back to treating as browser local when tz is empty', () => {
    const result = localDatetimeToUtcIso('2026-06-01T10:00', '');
    expect(result).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/);
  });

  it('handles half-hour offset: Asia/Kolkata (UTC+5:30)', () => {
    // 09:30 Kolkata = 04:00 UTC
    const result = localDatetimeToUtcIso('2026-06-01T09:30', 'Asia/Kolkata');
    expect(result).toBe('2026-06-01T04:00:00.000Z');
  });

  it('handles negative offset: America/New_York EST (UTC-5, winter)', () => {
    // 09:30 New York EST (winter) = 14:30 UTC
    const result = localDatetimeToUtcIso('2026-01-15T09:30', 'America/New_York');
    expect(result).toBe('2026-01-15T14:30:00.000Z');
  });

  it('handles negative offset: America/New_York EDT (UTC-4, summer)', () => {
    // 09:30 New York EDT (summer) = 13:30 UTC
    const result = localDatetimeToUtcIso('2026-06-15T09:30', 'America/New_York');
    expect(result).toBe('2026-06-15T13:30:00.000Z');
  });

  it('handles Australia/Sydney AEDT (UTC+11, summer)', () => {
    // 09:30 Sydney summer = 22:30 UTC previous day
    const result = localDatetimeToUtcIso('2026-01-15T09:30', 'Australia/Sydney');
    expect(result).toBe('2026-01-14T22:30:00.000Z');
  });
});

describe('utcIsoToDatetimeLocal — extended timezone coverage', () => {
  it('converts UTC to Asia/Kolkata (UTC+5:30)', () => {
    // 04:00 UTC = 09:30 Kolkata
    const result = utcIsoToDatetimeLocal('2026-06-01T04:00:00Z', 'Asia/Kolkata');
    expect(result).toBe('2026-06-01T09:30');
  });

  it('converts UTC to America/New_York EDT (UTC-4, summer)', () => {
    // 13:30 UTC = 09:30 New York EDT
    const result = utcIsoToDatetimeLocal('2026-06-15T13:30:00Z', 'America/New_York');
    expect(result).toBe('2026-06-15T09:30');
  });

  it('converts UTC midnight correctly — no 24:00 bug', () => {
    // Midnight UTC should produce 00:00 not 24:00
    const result = utcIsoToDatetimeLocal('2026-06-01T00:00:00Z', 'UTC');
    expect(result).toBe('2026-06-01T00:00');
    expect(result).not.toContain('24:00');
  });

  it('converts UTC to Europe/London GMT (UTC+0, winter)', () => {
    // In winter, London is UTC+0
    const result = utcIsoToDatetimeLocal('2026-01-15T10:00:00Z', 'Europe/London');
    expect(result).toBe('2026-01-15T10:00');
  });
});
