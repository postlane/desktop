// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Wizard from './wizard/Wizard';
import AddRepoModal from './wizard/AddRepoModal';
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
}: {
  view: ViewSelection;
  settingsOpen: boolean;
  postWizardNudge: boolean;
  onCloseSettings: () => void;
  onNudgeDismissed: () => void;
  onNavigateToRepo: (repoId: string) => void;
  onTimezoneChange: (tz: string) => void;
}) {
  if (settingsOpen) return <SettingsPanel onClose={onCloseSettings} onTimezoneChange={onTimezoneChange} />;

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

export default function App() {
  const [currentView, setCurrentView] = useState<ViewSelection>(DEFAULT_VIEW);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [showAddRepo, setShowAddRepo] = useState(false);
  const [timezone, setTimezone] = useState<string>('');

  // Load timezone from app state on startup
  useEffect(() => {
    invoke<AppStateFile>('read_app_state_command')
      .then((s) => setTimezone(s.timezone ?? ''))
      .catch(console.error);
  }, []);

  // Cmd+H / Ctrl+H — navigate to All repos Published
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'h') {
        e.preventDefault();
        setCurrentView({ view: 'all_repos', repoId: null, section: 'published' });
        setSettingsOpen(false);
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, []);
  const [postWizardNudge, setPostWizardNudge] = useState(false);
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Determine wizard visibility on launch
  useEffect(() => {
    Promise.all([
      invoke<AppStateFile>('read_app_state_command'),
      invoke<RepoWithStatus[]>('get_repos'),
    ])
      .then(([appState, repos]) => {
        if (!appState.wizard_completed && repos.length === 0) {
          setShowWizard(true);
        }
      })
      .catch(console.error);
  }, []);

  async function handleWizardComplete() {
    try {
      const appState = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', {
        state: { ...appState, wizard_completed: true },
      });
    } catch (e) {
      console.error('Failed to mark wizard complete:', e);
    }
    setShowWizard(false);
    setPostWizardNudge(true);
  }

  // Persist window dimensions on resize
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
            state: {
              ...appState,
              window: { width: size.width, height: size.height, x: pos.x, y: pos.y },
            },
          });
        } catch (e) {
          console.error('Failed to persist window size:', e);
        }
      }, 500);
    }).then((fn) => { unlisten = fn; }).catch(console.error);

    return () => {
      unlisten?.();
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
    };
  }, []);

  if (showWizard) {
    return <Wizard onComplete={handleWizardComplete} />;
  }

  return (
    <TimezoneContext.Provider value={timezone}>
    <div className="flex h-screen overflow-hidden bg-white dark:bg-zinc-900">
      <LeftNav
        currentView={currentView}
        onNavigate={(sel) => { setCurrentView(sel); setSettingsOpen(false); }}
        onSettingsOpen={() => setSettingsOpen(true)}
        onAddRepo={() => setShowAddRepo(true)}
      />
      {showAddRepo && (
        <AddRepoModal onClose={() => setShowAddRepo(false)} />
      )}
      <main className="flex-1 overflow-y-auto">
        <MainContent
          view={currentView}
          settingsOpen={settingsOpen}
          postWizardNudge={postWizardNudge}
          onCloseSettings={() => setSettingsOpen(false)}
          onNudgeDismissed={() => setPostWizardNudge(false)}
          onTimezoneChange={setTimezone}
          onNavigateToRepo={(repoId) => {
            setCurrentView({ view: 'repo', repoId, section: 'published' });
            setSettingsOpen(false);
          }}
        />
      </main>
    </div>
    </TimezoneContext.Provider>
  );
}
