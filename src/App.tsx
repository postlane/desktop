// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
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
