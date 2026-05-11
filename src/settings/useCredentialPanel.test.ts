// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';
import { useCredentialPanel } from './useCredentialPanel';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

const MASK = (raw: string) => `••••${raw.slice(-4)}`;

describe('useCredentialPanel — initial load', () => {
  it('sets configured state when credential exists', async () => {
    mockInvoke.mockResolvedValue('abcdefgh');
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('configured'));
    expect(result.current.preview).toBe('••••efgh');
  });

  it('sets idle state when credential not found', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('idle'));
  });
});

describe('useCredentialPanel — save', () => {
  it('returns true and updates preview on success', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('idle'));
    let ok: boolean | undefined;
    await act(async () => { ok = await result.current.saveCredential('abcdefgh'); });
    expect(ok).toBe(true);
    expect(result.current.preview).toBe('••••efgh');
    expect(result.current.panelState).toBe('configured');
  });

  it('returns false and sets saveError on failure', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') throw new Error('Keychain locked');
      return null;
    });
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('idle'));
    let ok: boolean | undefined;
    await act(async () => { ok = await result.current.saveCredential('abcdefgh'); });
    expect(ok).toBe(false);
    expect(result.current.saveError).toMatch(/keychain locked/i);
  });
});

describe('useCredentialPanel — test', () => {
  it('sets testResult ok on success', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'abcdefgh';
      if (cmd === 'test_scheduler') return true;
      return null;
    });
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('configured'));
    await act(() => result.current.handleTest());
    expect(result.current.testResult).toBe('ok');
  });
});

describe('useCredentialPanel — remove', () => {
  it('returns to idle and clears preview on success', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'abcdefgh';
      if (cmd === 'delete_scheduler_credential') return null;
      return null;
    });
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('configured'));
    await act(() => result.current.handleRemove());
    expect(result.current.panelState).toBe('idle');
    expect(result.current.preview).toBeNull();
  });

  it('sets removeError on failure', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'abcdefgh';
      if (cmd === 'delete_scheduler_credential') throw new Error('Keychain locked');
      return null;
    });
    const { result } = renderHook(() => useCredentialPanel({ provider: 'webhook', maskCredential: MASK }));
    await waitFor(() => expect(result.current.panelState).toBe('configured'));
    await act(() => result.current.handleRemove());
    expect(result.current.removeError).toMatch(/keychain locked/i);
  });
});
