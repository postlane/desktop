// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';

interface WizardOptions {
  startAt?: number;
  initialWorkspaceId?: string;
  initialWorkspaceName?: string;
}

interface State {
  step: number;
  token: string | null;
  provider: string | null;
  workspaceId: string | null;
  workspaceName: string | null;
  schedulerLinked: boolean;
  complete: boolean;
}

function canAdvance(s: State): boolean {
  if (s.step === 2) return s.token !== null;
  return true;
}

function applyNext(s: State): State {
  if (!canAdvance(s)) return s;
  if (s.step === 7) return { ...s, complete: true };
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
    provider: null,
    workspaceId: options.initialWorkspaceId ?? null,
    workspaceName: options.initialWorkspaceName ?? null,
    schedulerLinked: false,
    complete: false,
  });

  return {
    step: state.step,
    provider: state.provider,
    workspaceId: state.workspaceId,
    workspaceName: state.workspaceName,
    schedulerLinked: state.schedulerLinked,
    complete: state.complete,
    canGoBack: state.step > 1,
    next: () => setState(applyNext),
    back: () => setState(applyBack),
    skip: () => setState(applySkip),
    setToken: (token: string) => setState((s) => ({ ...s, token })),
    setProvider: (p: string) => setState((s) => ({ ...s, provider: p })),
    setWorkspaceId: (id: string) => setState((s) => ({ ...s, workspaceId: id })),
    setWorkspaceName: (name: string) => setState((s) => ({ ...s, workspaceName: name })),
    setSchedulerLinked: (v: boolean) => setState((s) => ({ ...s, schedulerLinked: v })),
  };
}
