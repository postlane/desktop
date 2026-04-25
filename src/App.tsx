// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Wizard from './wizard/Wizard';
import AddRepoModal from './wizard/AddRepoModal';
import TelemetryConsentModal from './telemetry/TelemetryConsentModal';
import LeftNav from './nav/LeftNav';
import AllReposDraftsView from './drafts/AllReposDraftsView';
import AllReposPublishedView from './published/AllReposPublishedView';
import RepoDraftsView from './drafts/RepoDraftsView';
import RepoPublishedView from './published/RepoPublishedView';
import SettingsPanel from './settings/SettingsPanel';
import { TimezoneContext } from './TimezoneContext';
import type { AppStateFile, RepoWithStatus, ViewSelection } from './types';

const DEFAULT_VIEW: ViewSelection = {
  view: 'all_repos',
  repoId: null,
  section: 'drafts',
};

function MainContent({
  view,
  settingsOpen,
  postWizardNudge,
  onCloseSettings,
  onNudgeDismissed,
  onNavigateToRepo,
  onTimezoneChange,
  onRepoChange,
}: {
  view: ViewSelection;
  settingsOpen: boolean;
  postWizardNudge: boolean;
  onCloseSettings: () => void;
  onNudgeDismissed: () => void;
  onNavigateToRepo: (_repoId: string) => void;
  onTimezoneChange: (_tz: string) => void;
  onRepoChange: () => void;
}) {
  if (settingsOpen) return <SettingsPanel onClose={onCloseSettings} onTimezoneChange={onTimezoneChange} onRepoChange={onRepoChange} />;
  if (view.view === 'all_repos') {
    return view.section === 'published'
      ? <AllReposPublishedView onNavigateToRepo={onNavigateToRepo} />
      : <AllReposDraftsView postWizardNudge={postWizardNudge} onNudgeDismissed={onNudgeDismissed} />;
  }
  if (!view.repoId) return <AllReposDraftsView postWizardNudge={false} onNudgeDismissed={onNudgeDismissed} />;
  return view.section === 'published'
    ? <RepoPublishedView repoId={view.repoId} />
    : <RepoDraftsView repoId={view.repoId} />;
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
        } catch (e) { console.error('Failed to persist window size:', e); }
      }, 500);
    }).then((fn) => { unlisten = fn; }).catch(console.error);
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
  const [showWizard, setShowWizard] = useState(false);
  const [showAddRepo, setShowAddRepo] = useState(false);
  const [showConsentModal, setShowConsentModal] = useState(false);
  const [timezone, setTimezone] = useState<string>('');
  const [repoVersion, setRepoVersion] = useState(0);
  const [postWizardNudge, setPostWizardNudge] = useState(false);

  useEffect(() => {
    invoke<AppStateFile>('read_app_state_command')
      .then((s) => { setTimezone(s.timezone ?? ''); if (!s.consent_asked) setShowConsentModal(true); })
      .catch(console.error);
  }, []);

  useEffect(() => {
    Promise.all([invoke<AppStateFile>('read_app_state_command'), invoke<RepoWithStatus[]>('get_repos')])
      .then(([appState, repos]) => { if (!appState.wizard_completed && repos.length === 0) setShowWizard(true); })
      .catch(console.error);
  }, []);

  async function handleWizardComplete() {
    try {
      const appState = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...appState, wizard_completed: true } });
    } catch (e) { console.error('Failed to mark wizard complete:', e); }
    setShowWizard(false); setPostWizardNudge(true);
  }

  async function handleConsentChoice(consent: boolean) {
    try { await invoke('set_telemetry_consent', { consent }); }
    catch (e) { console.error('set_telemetry_consent failed:', e); }
    setShowConsentModal(false);
  }

  return {
    currentView, setCurrentView, settingsOpen, setSettingsOpen, showWizard, showAddRepo,
    setShowAddRepo, showConsentModal, timezone, setTimezone, repoVersion, setRepoVersion,
    postWizardNudge, setPostWizardNudge, handleWizardComplete, handleConsentChoice,
  };
}

export default function App() {
  const {
    currentView, setCurrentView, settingsOpen, setSettingsOpen, showWizard, showAddRepo,
    setShowAddRepo, showConsentModal, timezone, setTimezone, repoVersion, setRepoVersion,
    postWizardNudge, setPostWizardNudge, handleWizardComplete, handleConsentChoice,
  } = useAppState();

  useCmdHShortcut(() => { setCurrentView({ view: 'all_repos', repoId: null, section: 'published' }); setSettingsOpen(false); });
  useWindowSizePersistence();

  if (showWizard) return <Wizard onComplete={handleWizardComplete} />;

  return (
    <TimezoneContext.Provider value={timezone}>
      <div className="flex h-screen overflow-hidden bg-white dark:bg-zinc-900">
        <LeftNav
          currentView={currentView}
          onNavigate={(sel) => { setCurrentView(sel); setSettingsOpen(false); }}
          onSettingsOpen={() => setSettingsOpen(true)}
          onAddRepo={() => setShowAddRepo(true)}
          refreshKey={repoVersion}
        />
        {showConsentModal && <TelemetryConsentModal onAccept={() => handleConsentChoice(true)} onDecline={() => handleConsentChoice(false)} />}
        {showAddRepo && <AddRepoModal onClose={() => setShowAddRepo(false)} />}
        <main className="flex-1 overflow-y-auto">
          <MainContent
            view={currentView} settingsOpen={settingsOpen} postWizardNudge={postWizardNudge}
            onCloseSettings={() => setSettingsOpen(false)} onNudgeDismissed={() => setPostWizardNudge(false)}
            onTimezoneChange={setTimezone} onRepoChange={() => setRepoVersion((v) => v + 1)}
            onNavigateToRepo={(repoId) => { setCurrentView({ view: 'repo', repoId, section: 'published' }); setSettingsOpen(false); }}
          />
        </main>
      </div>
    </TimezoneContext.Provider>
  );
}
