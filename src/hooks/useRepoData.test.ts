// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';

// Mock both invoke sources so we can detect which one is actually called
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { invoke as wrapperInvoke } from '../ipc/invoke';

const mockTauriInvoke = vi.mocked(tauriInvoke);
const mockWrapperInvoke = vi.mocked(wrapperInvoke);

import { useRepoData, useProjectRepos } from './useRepoData';

beforeEach(() => {
  vi.clearAllMocks();
});

describe('useRepoData', () => {
  it('uses wrapper invoke (not bare tauri invoke) so session_expired is intercepted', async () => {
    mockWrapperInvoke.mockResolvedValue([]);
    const { result } = renderHook(() => useRepoData());
    await waitFor(() => expect(result.current.repos).toBeDefined());

    // The wrapper invoke should have been called, NOT the bare tauri invoke
    expect(mockWrapperInvoke).toHaveBeenCalled();
    expect(mockTauriInvoke).not.toHaveBeenCalled();
  });

  it('sets loadError when refresh fails (§review-silentcatch)', async () => {
    mockWrapperInvoke
      .mockResolvedValueOnce([]) // initial load
      .mockRejectedValueOnce(new Error('Connection refused')); // refresh
    const { result } = renderHook(() => useRepoData());
    await waitFor(() => expect(result.current.repos).toBeDefined());
    await act(() => result.current.refresh());
    await waitFor(() =>
      expect(result.current.loadError).toBe('Could not load repositories. Check logs.'),
    );
  });
});

describe('useProjectRepos', () => {
  it('sets loadError when refresh fails (§review-silentcatch)', async () => {
    mockWrapperInvoke
      .mockResolvedValueOnce([]) // initial load
      .mockRejectedValueOnce(new Error('Connection refused')); // refresh
    const { result } = renderHook(() => useProjectRepos('proj-1'));
    await waitFor(() => expect(result.current.repos).toBeDefined());
    await act(() => result.current.refresh());
    await waitFor(() =>
      expect(result.current.loadError).toBe('Could not load repositories. Check logs.'),
    );
  });
});
