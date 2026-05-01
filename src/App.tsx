// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Wizard from './wizard/Wizard';
import SignInScreen from './wizard/SignInScreen';
import AddRepoModal from './wizard/AddRepoModal';
import TelemetryConsentModal from './telemetry/TelemetryConsentModal';
import LeftNav from './nav/LeftNav';
import AllReposDraftsView from './drafts/AllReposDraftsView';
import AllReposPublishedView from './published/AllReposPublishedView';
import RepoDraftsView from './drafts/RepoDraftsView';
import RepoPublishedView from './published/RepoPublishedView';
import SettingsPanel from './settings/SettingsPanel';
import { TimezoneContext } from './TimezoneContext';
import type { AppStateFile, ViewSelection } from './types';

const DEFAULT_VIEW: ViewSelection = {
  view: 'all_repos',
  repoId: null,
  section: 'drafts',
};

function MainContent({
  view,
  settingsOpen,
  schedulerTab,
  postWizardNudge,
  onCloseSettings,
  onNudgeDismissed,
  onNavigateToRepo,
  onTimezoneChange,
  onRepoChange,
  onOpenSchedulerSettings,
}: {
  view: ViewSelection;
  settingsOpen: boolean;
  schedulerTab: boolean;
  postWizardNudge: boolean;
  onCloseSettings: () => void;
  onNudgeDismissed: () => void;
  onNavigateToRepo: (_repoId: string) => void;
  onTimezoneChange: (_tz: string) => void;
  onRepoChange: () => void;
  onOpenSchedulerSettings: () => void;
}) {
  if (settingsOpen) return (
    <SettingsPanel
      onClose={onCloseSettings}
      onTimezoneChange={onTimezoneChange}
      onRepoChange={onRepoChange}
      initialTab={schedulerTab ? 'scheduler' : undefined}
      onAddWorkspace={undefined}
      onAddRepo={undefined}
    />
  );
  if (view.view === 'all_repos') {
    return view.section === 'published'
      ? <AllReposPublishedView onNavigateToRepo={onNavigateToRepo} />
      : <AllReposDraftsView postWizardNudge={postWizardNudge} onNudgeDismissed={onNudgeDismissed} />;
  }
  if (!view.repoId) return <AllReposDraftsView postWizardNudge={false} onNudgeDismissed={onNudgeDismissed} />;
  return view.section === 'published'
    ? <RepoPublishedView repoId={view.repoId} />
    : <RepoDraftsView repoId={view.repoId} onOpenSchedulerSettings={onOpenSchedulerSettings} />;
}

function useWindowSizePersistence() {
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    win.onResized(async ({ payload: size }) => {
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
      resizeTimerRef.current = setTimeout(async () => {
        try {
          const pos = await win.outerPosition();
          const appState = await invoke<AppStateFile>('read_app_state_command');
          await invoke('save_app_state_command', {
            state: { ...appState, window: { width: size.width, height: size.height, x: pos.x, y: pos.y } },
          });
        } catch (e) { console.error('Failed to persist window size:', e instanceof Error ? e.message : String(e)); }
      }, 500);
    }).then((fn) => { unlisten = fn; }).catch((e: unknown) => console.error('Failed to set up resize listener:', e instanceof Error ? e.message : String(e)));
    return () => { unlisten?.(); if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current); };
  }, []);
}

function useCmdHShortcut(onActivate: () => void) {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'h') { e.preventDefault(); onActivate(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [onActivate]);
}

function useAppState() {
  const [currentView, setCurrentView] = useState<ViewSelection>(DEFAULT_VIEW);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [schedulerTab, setSchedulerTab] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [showSignIn, setShowSignIn] = useState(false);
  const [showAddRepo, setShowAddRepo] = useState(false);
  const [showConsentModal, setShowConsentModal] = useState(false);
  const [timezone, setTimezone] = useState<string>('');
  const [repoVersion, setRepoVersion] = useState(0);
  const [postWizardNudge, setPostWizardNudge] = useState(false);

  useEffect(() => {
    Promise.all([
      invoke<AppStateFile>('read_app_state_command'),
      invoke<boolean>('get_license_signed_in'),
    ])
      .then(([appState, hasToken]) => {
        setTimezone(appState.timezone ?? '');
        if (!appState.consent_asked) setShowConsentModal(true);
        if (!appState.wizard_completed) { setShowWizard(true); return; }
        if (!hasToken) { setShowSignIn(true); }
      })
      .catch(console.error);
  }, []);

  function handleWizardComplete() {
    setShowWizard(false);
    setPostWizardNudge(true);
  }

  function handleSignedIn() {
    setShowSignIn(false);
  }

  async function handleConsentChoice(consent: boolean) {
    try { await invoke('set_telemetry_consent', { consent }); }
    catch (e) { console.error('set_telemetry_consent failed:', e); }
    setShowConsentModal(false);
  }

  function openSettings() { setSettingsOpen(true); setSchedulerTab(false); }
  function openSchedulerSettings() { setSettingsOpen(true); setSchedulerTab(true); }
  function closeSettings() { setSettingsOpen(false); setSchedulerTab(false); }

  return {
    currentView, setCurrentView, settingsOpen, schedulerTab, showWizard, showSignIn,
    showAddRepo, setShowAddRepo, showConsentModal, timezone, setTimezone,
    repoVersion, setRepoVersion, postWizardNudge, setPostWizardNudge,
    handleWizardComplete, handleSignedIn, handleConsentChoice,
    openSettings, openSchedulerSettings, closeSettings,
  };
}

export default function App() {
  const {
    currentView, setCurrentView, settingsOpen, schedulerTab, showWizard, showSignIn,
    showAddRepo, setShowAddRepo, showConsentModal, timezone, setTimezone,
    repoVersion, setRepoVersion, postWizardNudge, setPostWizardNudge,
    handleWizardComplete, handleSignedIn, handleConsentChoice,
    openSettings, openSchedulerSettings, closeSettings,
  } = useAppState();

  useCmdHShortcut(() => { setCurrentView({ view: 'all_repos', repoId: null, section: 'published' }); closeSettings(); });
  useWindowSizePersistence();

  if (showWizard) return <Wizard onComplete={handleWizardComplete} />;
  if (showSignIn) return <SignInScreen onSignedIn={handleSignedIn} />;

  return (
    <TimezoneContext.Provider value={timezone}>
      <div className="flex h-screen overflow-hidden bg-white dark:bg-zinc-900">
        <LeftNav
          currentView={currentView}
          onNavigate={(sel) => { setCurrentView(sel); closeSettings(); }}
          onSettingsOpen={openSettings}
          onAddRepo={() => setShowAddRepo(true)}
          refreshKey={repoVersion}
        />
        {showConsentModal && <TelemetryConsentModal onAccept={() => handleConsentChoice(true)} onDecline={() => handleConsentChoice(false)} />}
        {showAddRepo && <AddRepoModal onClose={() => setShowAddRepo(false)} />}
        <main className="flex-1 overflow-y-auto">
          <MainContent
            view={currentView} settingsOpen={settingsOpen} schedulerTab={schedulerTab}
            postWizardNudge={postWizardNudge} onCloseSettings={closeSettings}
            onNudgeDismissed={() => setPostWizardNudge(false)} onTimezoneChange={setTimezone}
            onRepoChange={() => setRepoVersion((v) => v + 1)} onOpenSchedulerSettings={openSchedulerSettings}
            onNavigateToRepo={(repoId) => { setCurrentView({ view: 'repo', repoId, section: 'published' }); closeSettings(); }}
          />
        </main>
      </div>
    </TimezoneContext.Provider>
  );
}
