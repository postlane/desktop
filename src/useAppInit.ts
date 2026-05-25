// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef, type MutableRefObject } from 'react';
import { invoke } from './ipc/invoke';
import { listen } from '@tauri-apps/api/event';
import type { AppStateFile } from './types';

export interface AppInitState {
  timezone: string;
  setTimezone: (_tz: string) => void;
  showConsentModal: boolean;
  setShowConsentModal: (_v: boolean) => void;
  showWizard: boolean;
  setShowWizard: (_v: boolean) => void;
  wizardStartStep: number;
  setWizardStartStep: (_step: number) => void;
  resumeStep: number | null;
  setResumeStep: (_step: number | null) => void;
  wizardWorkspaceId: string | null;
  wizardWorkspaceName: string | null;
  wizardProvider: string | null;
  showReSignIn: boolean;
  setShowReSignIn: (_v: boolean) => void;
  wizardNudgePending: boolean;
  setWizardNudgePending: (_v: boolean) => void;
  initError: string | null;
  appStateRef: MutableRefObject<AppStateFile | null>;
}

interface WizardSetters {
  setWizardStartStep: (_step: number) => void;
  setResumeStep: (_step: number | null) => void;
  setWizardWorkspaceId: (_id: string | null) => void;
  setWizardWorkspaceName: (_name: string | null) => void;
  setWizardProvider: (_p: string | null) => void;
  setShowWizard: (_v: boolean) => void;
}

async function applyWizardResumeState(setters: WizardSetters): Promise<void> {
  const saved = await invoke<{ step: number; workspaceId?: string; workspaceName?: string; provider?: string } | null>(
    'read_wizard_state'
  ).catch(() => null);
  if (saved && saved.step > 1) {
    setters.setWizardStartStep(saved.step);
    setters.setResumeStep(saved.step);
    setters.setWizardWorkspaceId(saved.workspaceId ?? null);
    setters.setWizardWorkspaceName(saved.workspaceName ?? null);
    setters.setWizardProvider(saved.provider ?? null);
  }
  setters.setShowWizard(true);
}

export function useAppInit(): AppInitState {
  const [timezone, setTimezone] = useState<string>(() => Intl.DateTimeFormat().resolvedOptions().timeZone);
  const [showConsentModal, setShowConsentModal] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [wizardStartStep, setWizardStartStep] = useState(1);
  const [resumeStep, setResumeStep] = useState<number | null>(null);
  const [wizardWorkspaceId, setWizardWorkspaceId] = useState<string | null>(null);
  const [wizardWorkspaceName, setWizardWorkspaceName] = useState<string | null>(null);
  const [wizardProvider, setWizardProvider] = useState<string | null>(null);
  const [showReSignIn, setShowReSignIn] = useState(false);
  const [wizardNudgePending, setWizardNudgePending] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);
  const appStateRef = useRef<AppStateFile | null>(null);

  useEffect(() => {
    const unlisten = listen('license:expired', () => setShowReSignIn(true));
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  useEffect(() => {
    const setters: WizardSetters = {
      setWizardStartStep, setResumeStep, setWizardWorkspaceId, setWizardWorkspaceName,
      setWizardProvider, setShowWizard,
    };
    Promise.all([
      invoke<AppStateFile>('read_app_state_command'),
      invoke<boolean>('get_license_signed_in'),
    ])
      .then(async ([appState, hasToken]) => {
        appStateRef.current = appState;
        const tz = appState.timezone || Intl.DateTimeFormat().resolvedOptions().timeZone;
        setTimezone(tz);
        if (!appState.timezone) {
          invoke('save_app_state_command', { state: { ...appState, timezone: tz } }).catch(console.error);
        }
        if (!appState.consent_asked) setShowConsentModal(true);
        if (!appState.wizard_completed) {
          const hasActiveRepos = await invoke<boolean>('has_active_repos').catch(() => false);
          if (!hasActiveRepos) { await applyWizardResumeState(setters); return; }
          // Active repos exist — skip the wizard and go to the main app
        }
        if (!hasToken) { setShowReSignIn(true); return; }
        if (!appState.post_wizard_completed) setWizardNudgePending(true);
      })
      .catch((e: unknown) => setInitError(e instanceof Error ? e.message : String(e)));
  }, []);

  return {
    timezone, setTimezone, showConsentModal, setShowConsentModal,
    showWizard, setShowWizard, wizardStartStep, setWizardStartStep,
    resumeStep, setResumeStep, wizardWorkspaceId, wizardWorkspaceName, wizardProvider,
    showReSignIn, setShowReSignIn, wizardNudgePending, setWizardNudgePending,
    initError, appStateRef,
  };
}
