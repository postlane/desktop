// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';

interface WizardOptions {
  startAt?: number;
}

interface State {
  step: number;
  token: string | null;
  workspaceId: string | null;
  schedulerLinked: boolean;
  repoAdded: boolean;
  complete: boolean;
}

function canAdvance(s: State): boolean {
  if (s.step === 2) return s.token !== null;
  if (s.step === 5) return s.repoAdded;
  return true;
}

function applyNext(s: State): State {
  if (!canAdvance(s)) return s;
  if (s.step === 5) return { ...s, complete: true };
  return { ...s, step: s.step + 1 };
}

function applyBack(s: State): State {
  if (s.step <= 1) return s;
  return { ...s, step: s.step - 1 };
}

function applySkip(s: State): State {
  if (s.step === 4) return { ...s, schedulerLinked: false, step: 5 };
  return s;
}

export function useWizardState(options: WizardOptions = {}) {
  const [state, setState] = useState<State>({
    step: options.startAt ?? 1,
    token: null,
    workspaceId: null,
    schedulerLinked: false,
    repoAdded: false,
    complete: false,
  });

  return {
    step: state.step,
    workspaceId: state.workspaceId,
    schedulerLinked: state.schedulerLinked,
    complete: state.complete,
    canGoBack: state.step > 1,
    next: () => setState(applyNext),
    back: () => setState(applyBack),
    skip: () => setState(applySkip),
    setToken: (token: string) => setState((s) => ({ ...s, token })),
    setWorkspaceId: (id: string) => setState((s) => ({ ...s, workspaceId: id })),
    setRepoAdded: (v: boolean) => setState((s) => ({ ...s, repoAdded: v })),
    setSchedulerLinked: (v: boolean) => setState((s) => ({ ...s, schedulerLinked: v })),
  };
}
