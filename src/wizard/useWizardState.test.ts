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

// step 5 = GitHub App install; step 6 = voice guide; step 7 = complete
describe('useWizardState — step 5 GitHub App', () => {
  it('test_next_on_step_5_advances_to_step_6', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.next()); // → 4
    act(() => result.current.skip()); // skip scheduler → 5
    act(() => result.current.next()); // → 6
    expect(result.current.step).toBe(6);
    expect(result.current.complete).toBe(false);
  });

  it('test_back_from_step_5_goes_to_step_4', () => {
    const { result } = renderHook(() => useWizardState({ startAt: 5 }));
    act(() => result.current.back());
    expect(result.current.step).toBe(4);
  });
});

describe('useWizardState — step 6 voice guide', () => {
  it('test_next_on_step_6_advances_to_step_7', () => {
    const { result } = renderHook(() => useWizardState({ startAt: 6 }));
    act(() => result.current.next()); // → 7
    expect(result.current.step).toBe(7);
    expect(result.current.complete).toBe(false);
  });

  it('test_back_from_step_6_goes_to_step_5', () => {
    const { result } = renderHook(() => useWizardState({ startAt: 6 }));
    act(() => result.current.back());
    expect(result.current.step).toBe(5);
  });
});

describe('useWizardState — step 7 completion', () => {
  it('test_next_on_step_7_sets_complete', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.next()); // → 4
    act(() => result.current.skip()); // skip scheduler → 5
    act(() => result.current.next()); // → 6
    act(() => result.current.next()); // → 7
    act(() => result.current.next()); // → complete
    expect(result.current.complete).toBe(true);
  });
});

describe('useWizardState — back boundary', () => {
  it('test_back_on_step_1_does_not_change_step', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.back());
    expect(result.current.step).toBe(1);
  });
});

describe('useWizardState — skip no-op on non-step-4', () => {
  it('test_skip_on_step_1_does_nothing', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.skip());
    expect(result.current.step).toBe(1);
    expect(result.current.schedulerLinked).toBe(false);
  });

  it('test_skip_on_step_3_does_nothing', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.next()); // → 2
    act(() => result.current.setToken('tok'));
    act(() => result.current.next()); // → 3
    act(() => result.current.skip());
    expect(result.current.step).toBe(3);
  });
});

describe('useWizardState — setter functions', () => {
  it('test_set_provider_updates_provider', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.setProvider('github'));
    expect(result.current.provider).toBe('github');
  });

  it('test_set_workspace_name_updates_workspace_name', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.setWorkspaceName('My Workspace'));
    expect(result.current.workspaceName).toBe('My Workspace');
  });

  it('test_set_scheduler_linked_true_updates_scheduler_linked', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.setSchedulerLinked(true));
    expect(result.current.schedulerLinked).toBe(true);
  });

  it('test_set_scheduler_linked_false_updates_scheduler_linked', () => {
    const { result } = renderHook(() => useWizardState());
    act(() => result.current.setSchedulerLinked(true));
    act(() => result.current.setSchedulerLinked(false));
    expect(result.current.schedulerLinked).toBe(false);
  });
});
