// SPDX-License-Identifier: BUSL-1.1

import { createContext, useContext } from 'react';

export const TimezoneContext = createContext<string>('');

export function useTimezone(): string {
  return useContext(TimezoneContext);
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
