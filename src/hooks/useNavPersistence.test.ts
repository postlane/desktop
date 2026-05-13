// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(),
}));

import { invoke } from '../ipc/invoke';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useNavPersistence } from './useNavPersistence';

const mockInvoke = vi.mocked(invoke);
const mockGetCurrentWindow = vi.mocked(getCurrentWindow);

const fakeWindow = {
  outerSize: vi.fn().mockResolvedValue({ width: 1200, height: 800 }),
  outerPosition: vi.fn().mockResolvedValue({ x: 100, y: 50 }),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  mockGetCurrentWindow.mockReturnValue(fakeWindow as unknown as ReturnType<typeof getCurrentWindow>);
  mockInvoke.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useNavPersistence — basic', () => {
  it('returns a function', () => {
    const { result } = renderHook(() => useNavPersistence());
    expect(typeof result.current).toBe('function');
  });

  it('debounces writes — does not call invoke immediately', () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(['r1']), { view: 'org_queue', projectId: 'p1' });
    });
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('calls invoke after debounce delay with correct state', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(['r1']), { view: 'org_queue', projectId: 'p1' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    expect(mockInvoke).toHaveBeenCalledWith('save_app_state_command', {
      state: {
        version: 1,
        window: { width: 1200, height: 800, x: 100, y: 50 },
        nav: {
          last_view: 'org_queue',
          last_repo_id: 'p1',
          last_section: '',
          expanded_repos: ['r1'],
        },
      },
    });
  });
});

describe('useNavPersistence — view-specific state', () => {
  it('sets projectId for org_history view', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(), { view: 'org_history', projectId: 'p2' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    const call = mockInvoke.mock.calls[0];
    expect((call[1] as { state: { nav: { last_repo_id: string | null } } }).state.nav.last_repo_id).toBe('p2');
  });

  it('sets null projectId for non-org views', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(), { view: 'no_orgs' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    const call = mockInvoke.mock.calls[0];
    expect((call[1] as { state: { nav: { last_repo_id: string | null } } }).state.nav.last_repo_id).toBeNull();
  });

  it('sets section for org_settings view', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(), { view: 'org_settings', projectId: 'p1', section: 'settings' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    const call = mockInvoke.mock.calls[0];
    expect((call[1] as { state: { nav: { last_section: string } } }).state.nav.last_section).toBe('settings');
  });

  it('sets section for global_settings view', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(), { view: 'global_settings', section: 'account' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    const call = mockInvoke.mock.calls[0];
    expect((call[1] as { state: { nav: { last_section: string } } }).state.nav.last_section).toBe('account');
  });
});

describe('useNavPersistence — debounce behavior', () => {
  it('cancels previous timer when called again within debounce window', async () => {
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(['r1']), { view: 'org_queue', projectId: 'p1' });
      vi.advanceTimersByTime(100);
      result.current(new Set(['r2']), { view: 'org_queue', projectId: 'p2' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    const call = mockInvoke.mock.calls[0];
    expect((call[1] as { state: { nav: { last_repo_id: string | null } } }).state.nav.last_repo_id).toBe('p2');
  });

  it('logs error and does not throw when invoke fails', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('IPC error'));
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { result } = renderHook(() => useNavPersistence());
    act(() => {
      result.current(new Set(), { view: 'org_queue', projectId: 'p1' });
    });
    await act(async () => { vi.advanceTimersByTime(300); });
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });
});
