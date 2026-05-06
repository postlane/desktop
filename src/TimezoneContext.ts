// SPDX-License-Identifier: BUSL-1.1

import { createContext, useContext } from 'react';

export const TimezoneContext = createContext<string>('');

export function useTimezone(): string {
  return useContext(TimezoneContext);
}

export function getTimezoneOffsetLabel(tz: string): string {
  if (!tz) return '';
  try {
    const parts = new Intl.DateTimeFormat('en', {
      timeZone: tz, timeZoneName: 'shortOffset',
    }).formatToParts(new Date());
    return parts.find((p) => p.type === 'timeZoneName')?.value ?? '';
  } catch { return ''; }
}

export function utcIsoToDatetimeLocal(isoUtc: string, tz: string): string {
  if (!isoUtc) return '';
  const date = new Date(isoUtc);
  const parts = new Intl.DateTimeFormat('en', {
    year: 'numeric', month: '2-digit', day: '2-digit',
    hour: '2-digit', minute: '2-digit', hour12: false,
    timeZone: tz || undefined,
  }).formatToParts(date);
  const get = (type: string) => parts.find((p) => p.type === type)?.value ?? '';
  return `${get('year')}-${get('month')}-${get('day')}T${get('hour')}:${get('minute')}`;
}

export function localDatetimeToUtcIso(localValue: string, tz: string): string {
  if (!tz) return new Date(localValue).toISOString();
  const dt = new Date(`${localValue}Z`);
  const utcStr = dt.toLocaleString('en-US', { timeZone: 'UTC' });
  const tzStr = dt.toLocaleString('en-US', { timeZone: tz });
  const offsetMs = new Date(utcStr).getTime() - new Date(tzStr).getTime();
  return new Date(dt.getTime() + offsetMs).toISOString();
}

/**
 * Format an ISO 8601 timestamp using the given IANA timezone.
 * Falls back to the browser's locale timezone if tz is empty.
 */
export function formatTimestamp(iso: string | null | undefined, tz: string): string {
  if (!iso) return '—';
  try {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
      timeZone: tz || undefined,
    }).format(new Date(iso));
  } catch {
    // Invalid timezone — fall back to locale default
    return new Date(iso).toLocaleString();
  }
}
