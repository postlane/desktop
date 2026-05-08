// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWizardState } from './useWizardState';

describe('useWizardState — initial state', () => {
  it('test_starts_at_step_1', () => {
    const { result } = renderHook(() => useWizardState());
    expect(result.current.step).toBe(1);
  });

  it('test_next_advances_step', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next());
    expect(result.current.step).toBe(2);
  });

  it('test_back_not_available_on_step_1', () => {
    const { result } = renderHook(() => useWizardState());
    expect(result.current.canGoBack).toBe(false);
  });

  it('test_add_workspace_entry_starts_at_step_3', () => {
    const { result } = renderHook(() => useWizardState({ startAt: 3 }));
    expect(result.current.step).toBe(3);
  });

  it('test_add_repo_entry_starts_at_step_5', () => {
    const { result } = renderHook(() => useWizardState({ startAt: 5 }));
    expect(result.current.step).toBe(5);
  });
});

describe('useWizardState — step 2 token gate', () => {
  it('test_next_blocked_on_step_2_without_token', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → step 2
    act(() => result.current.next()); // blocked — no token
    expect(result.current.step).toBe(2);
  });

  it('test_next_allowed_on_step_2_with_token', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → step 2
    act(() => result.current.setToken('tok-abc'));
    act(() => result.current.next()); // → step 3
    expect(result.current.step).toBe(3);
  });
});

describe('useWizardState — step 3 workspace', () => {
  it('test_next_on_step_3_stores_workspace_id', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok-abc'));
    act(() => result.current.next()); // → 3
    act(() => result.current.setWorkspaceId('ws-1'));
    act(() => result.current.next()); // → 4
    expect(result.current.workspaceId).toBe('ws-1');
    expect(result.current.step).toBe(4);
  });

  it('test_back_from_step_3_goes_to_step_2', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.back());
    expect(result.current.step).toBe(2);
  });
});

describe('useWizardState — step 4 scheduler', () => {
  it('test_skip_on_step_4_sets_scheduler_linked_false', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.next()); // → 4
    act(() => result.current.skip()); // skip scheduler → 5
    expect(result.current.schedulerLinked).toBe(false);
    expect(result.current.step).toBe(5);
  });

  it('test_back_from_step_4_goes_to_step_3', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.next()); // → 4
    act(() => result.current.back());
    expect(result.current.step).toBe(3);
  });
});

describe('useWizardState — step 5 completion', () => {
  it('test_next_on_step_5_sets_complete', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.next()); // → 4
    act(() => result.current.skip()); // → 5
    act(() => result.current.next()); // → complete
    expect(result.current.complete).toBe(true);
  });
});
