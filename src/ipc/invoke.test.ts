// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke, SESSION_EXPIRED_ERROR, registerSessionExpiredHandler } from './invoke';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
const mockTauriInvoke = vi.mocked(tauriInvoke);

beforeEach(() => {
  vi.resetAllMocks();
});

describe('invoke wrapper', () => {
  it('passes through successful results', async () => {
    mockTauriInvoke.mockResolvedValueOnce('result-value');
    const result = await invoke<string>('some_command');
    expect(result).toBe('result-value');
  });

  it('re-throws non-session-expired errors unchanged', async () => {
    mockTauriInvoke.mockRejectedValueOnce('some other error');
    await expect(invoke('some_command')).rejects.toBe('some other error');
  });

  it('calls registered handlers when session_expired error is thrown', async () => {
    mockTauriInvoke.mockRejectedValueOnce(SESSION_EXPIRED_ERROR);
    const handler = vi.fn();
    const unregister = registerSessionExpiredHandler(handler);

    await expect(invoke('any_command')).rejects.toBe(SESSION_EXPIRED_ERROR);
    expect(handler).toHaveBeenCalledOnce();
    unregister();
  });

  it('re-throws session_expired after calling handlers so callers can react', async () => {
    mockTauriInvoke.mockRejectedValueOnce(SESSION_EXPIRED_ERROR);
    const handler = vi.fn();
    const unregister = registerSessionExpiredHandler(handler);

    let caught: unknown = null;
    try {
      await invoke('any_command');
    } catch (e) {
      caught = e;
    }
    expect(caught).toBe(SESSION_EXPIRED_ERROR);
    unregister();
  });

  it('does not call handlers when unregistered before error fires', async () => {
    mockTauriInvoke.mockRejectedValueOnce(SESSION_EXPIRED_ERROR);
    const handler = vi.fn();
    const unregister = registerSessionExpiredHandler(handler);
    unregister();

    await expect(invoke('any_command')).rejects.toBe(SESSION_EXPIRED_ERROR);
    expect(handler).not.toHaveBeenCalled();
  });
});
