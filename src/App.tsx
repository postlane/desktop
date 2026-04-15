// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { AppStateFile } from './types';
import LeftNav from './nav/LeftNav';
import AllReposDrafts from './pages/AllReposDrafts';
import AllReposPublished from './pages/AllReposPublished';
import RepoDrafts from './pages/RepoDrafts';
import RepoPublished from './pages/RepoPublished';
import Settings from './pages/Settings';
import type { ViewSelection } from './types';

const DEFAULT_VIEW: ViewSelection = {
  view: 'all_repos',
  repoId: null,
  section: 'drafts',
};

function MainContent({
  view,
  settingsOpen,
  onCloseSettings,
}: {
  view: ViewSelection;
  settingsOpen: boolean;
  onCloseSettings: () => void;
}) {
  if (settingsOpen) return <Settings onClose={onCloseSettings} />;

  if (view.view === 'all_repos') {
    return view.section === 'published'
      ? <AllReposPublished />
      : <AllReposDrafts />;
  }

  if (!view.repoId) return <AllReposDrafts />;

  return view.section === 'published'
    ? <RepoPublished repoId={view.repoId} />
    : <RepoDrafts repoId={view.repoId} />;
}

export default function App() {
  const [currentView, setCurrentView] = useState<ViewSelection>(DEFAULT_VIEW);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  return (
    <div className="flex h-screen overflow-hidden bg-white dark:bg-zinc-900">
      <LeftNav
        currentView={currentView}
        onNavigate={(sel) => { setCurrentView(sel); setSettingsOpen(false); }}
        onSettingsOpen={() => setSettingsOpen(true)}
      />
      <main className="flex-1 overflow-y-auto">
        <MainContent
          view={currentView}
          settingsOpen={settingsOpen}
          onCloseSettings={() => setSettingsOpen(false)}
        />
      </main>
    </div>
  );
}
