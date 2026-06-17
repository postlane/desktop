// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useAppInit } from '../useAppInit';
import type { ViewSelection } from '../types';

const DEFAULT_VIEW: ViewSelection = { view: 'no_orgs' };

export function useAppState() {
  const [currentView, setCurrentView] = useState<ViewSelection>(DEFAULT_VIEW);
  const [repoVersion, setRepoVersion] = useState(0);
  const init = useAppInit();

  function handleWizardComplete() {
    invoke('set_wizard_completed').catch(console.error);
    init.setShowWizard(false); init.setWizardStartStep(1); init.setResumeStep(null);
  }
  function handleResumeDecline() {
    init.setWizardStartStep(1); init.setResumeStep(null);
    invoke('clear_wizard_state').catch(console.warn);
  }
  function handleAddOrg() { init.setWizardStartStep(2); init.setShowWizard(true); }
  function handleSignedIn() { init.setShowReSignIn(false); }
  function handleSignedOut() { init.setShowReSignIn(true); }

  async function handleConsentChoice(consent: boolean) {
    await invoke('set_telemetry_consent', { consent }).catch((e: unknown) => console.error('set_telemetry_consent failed:', e));
    init.setShowConsentModal(false);
  }

  function handleWizardNudgeHandled() {
    init.setWizardNudgePending(false);
    if (init.appStateRef.current) {
      invoke('save_app_state_command', { state: { ...init.appStateRef.current, post_wizard_completed: true } }).catch(console.error);
    }
  }

  return {
    currentView, setCurrentView,
    showWizard: init.showWizard, wizardStartStep: init.wizardStartStep,
    resumeStep: init.resumeStep, setResumeStep: init.setResumeStep,
    wizardWorkspaceId: init.wizardWorkspaceId, wizardWorkspaceName: init.wizardWorkspaceName, wizardProvider: init.wizardProvider,
    showReSignIn: init.showReSignIn, showConsentModal: init.showConsentModal,
    timezone: init.timezone, setTimezone: init.setTimezone,
    repoVersion, setRepoVersion, initError: init.initError,
    wizardNudgePending: init.wizardNudgePending,
    handleWizardComplete, handleResumeDecline, handleAddOrg,
    handleSignedIn, handleConsentChoice, handleWizardNudgeHandled, handleSignedOut,
  };
}
