// SPDX-License-Identifier: BUSL-1.1

// Typed wrapper around Tauri's `invoke` that intercepts `session_expired` errors.
// All Tauri IPC calls in M19+ must use this wrapper — never call @tauri-apps/api/core's
// invoke directly in components, or SessionExpired will silently pass through without
// navigating the user to AccountSettingsView.
//
// Wire-up sequence:
//  1. Rust command receives HTTP 401 → returns Err("session_expired")
//  2. This wrapper catches the error
//  3. Clears ProjectsProvider and DraftPostsProvider (wired in 19.1.6)
//  4. Navigates to AccountSettingsView
//  5. Re-throws so the calling component's catch block can react if needed

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { InvokeArgs } from '@tauri-apps/api/core';

export const SESSION_EXPIRED_ERROR = 'session_expired' as const;

// Callbacks registered by providers in 19.1.6 to clear their state on session expiry.
// Using callbacks avoids circular imports between this module and React contexts.
let onSessionExpired: (() => void)[] = [];

/** Register a callback to run when any command returns SESSION_EXPIRED_ERROR. */
export function registerSessionExpiredHandler(handler: () => void): () => void {
  onSessionExpired.push(handler);
  return () => {
    onSessionExpired = onSessionExpired.filter((h) => h !== handler);
  };
}

/** Invoke a Tauri command, intercepting session_expired errors. */
export async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    if (e === SESSION_EXPIRED_ERROR) {
      onSessionExpired.forEach((h) => h());
      throw e;
    }
    throw e;
  }
}
