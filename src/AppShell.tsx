// SPDX-License-Identifier: BUSL-1.1

import { useCallback } from 'react';
import TelemetryConsentModal from './telemetry/TelemetryConsentModal';
import LeftNav from './nav/LeftNav';
import AccountRail from './nav/AccountRail';
import { MainContent } from './components/MainContent';
import { EditGuardContext } from './context/EditGuardContext';
import type { useAppState } from './hooks/useAppState';
import type { useDirtyNavGuard } from './hooks/useAppHooks';

export function AppShell({
  appState,
  guard,
  showToast,
  toastMessage,
}: {
  appState: ReturnType<typeof useAppState>;
  guard: ReturnType<typeof useDirtyNavGuard>;
  showToast: (_msg: string) => void;
  toastMessage: string | null;
}) {
  const { currentView, showConsentModal, handleAddOrg,
    setRepoVersion, wizardNudgePending,
    handleConsentChoice, handleWizardNudgeHandled, handleSignedOut, setTimezone } = appState;
  const { discardModalOpen, handleNavClick, handleAccountSwitch, confirmDiscard, cancelDiscard, editPostViewDirtyRef, resetSignal } = guard;
  const setDirty = useCallback((dirty: boolean) => { editPostViewDirtyRef.current = dirty; }, [editPostViewDirtyRef]);
  return (
    <div className="is-flex" style={{ height: '100vh', overflow: 'hidden', background: 'white' }}>
      <AccountRail onSwitch={handleAccountSwitch} />
      <LeftNav currentView={currentView} onNavigate={handleNavClick}
        onSettingsOpen={() => handleNavClick({ view: 'global_settings', section: 'account' })}
        onAddWorkspace={handleAddOrg} />
      {showConsentModal && <TelemetryConsentModal onAccept={() => handleConsentChoice(true)} onDecline={() => handleConsentChoice(false)} />}
      {discardModalOpen && (
        <div className="modal is-active">
          <div className="modal-background" onClick={cancelDiscard} />
          <div className="modal-content box has-text-centered" style={{ maxWidth: '20rem' }}>
            <p className="mb-4 is-size-7">You have unsaved changes. Discard them?</p>
            <div className="buttons is-centered">
              <button className="button is-danger is-small" onClick={confirmDiscard}>Discard</button>
              <button className="button is-small" onClick={cancelDiscard}>Cancel</button>
            </div>
          </div>
        </div>
      )}
      {toastMessage && (
        <div className="notification is-success is-small" style={{ position: 'fixed', bottom: '1rem', right: '1rem', zIndex: 9999 }}>
          {toastMessage}
        </div>
      )}
      <EditGuardContext.Provider value={{ resetSignal, setDirty, pendingNavSel: null, onNavCancelled: cancelDiscard }}>
        <main style={{ flex: 1, overflowY: 'auto' }}>
          <MainContent key={JSON.stringify(currentView)} view={currentView}
            onNavigate={handleNavClick} onToast={showToast}
            onTimezoneChange={setTimezone} onRepoChange={() => setRepoVersion((v) => v + 1)}
            onSignedOut={handleSignedOut} wizardNudgePending={wizardNudgePending}
            onWizardNudgeHandled={handleWizardNudgeHandled}
          />
        </main>
      </EditGuardContext.Provider>
    </div>
  );
}
