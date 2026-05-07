// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import ReposTab from './ReposTab';
import SchedulerTab from './SchedulerTab';
import AnalyticsTab from './AnalyticsTab';
import AppTab from './AppTab';

type Tab = 'repos' | 'scheduler' | 'analytics' | 'app';

const TAB_LABELS: Record<Tab, string> = {
  repos: 'Repos',
  scheduler: 'Default scheduler',
  analytics: 'Analytics',
  app: 'App',
};

interface Props {
  onClose: () => void;
  onTimezoneChange?: (_tz: string) => void;
  onRepoChange?: () => void;
  activeRepoId?: string | null;
  initialTab?: Tab;
  onAddWorkspace?: () => void;
  onAddRepo?: () => void;
}

export default function SettingsPanel({ onClose, onTimezoneChange, onRepoChange, activeRepoId, initialTab, onAddWorkspace, onAddRepo }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>(initialTab ?? 'repos');

  return (
    <div className="is-flex" style={{ flexDirection: 'column', height: '100%', background: 'white' }}>
      <div className="is-flex is-align-items-center is-justify-content-space-between px-5 py-4" style={{ borderBottom: '1px solid var(--bulma-border-weak)' }}>
        <h1 className="has-text-weight-semibold">Settings</h1>
        <button className="button is-ghost is-small" onClick={onClose} aria-label="Close settings">✕</button>
      </div>
      <div className="tabs mb-0" role="tablist">
        <ul>
          {(['repos', 'scheduler', 'analytics', 'app'] as Tab[]).map((tab) => (
            <li key={tab} className={activeTab === tab ? 'is-active' : ''}>
              <a role="tab" aria-selected={activeTab === tab} onClick={() => setActiveTab(tab)}>
                {TAB_LABELS[tab]}
              </a>
            </li>
          ))}
        </ul>
      </div>
      <div className="p-5" style={{ flex: 1, overflowY: 'auto' }}>
        {activeTab === 'repos' && <ReposTab onRepoChange={() => onRepoChange?.()} onAddWorkspace={onAddWorkspace} onAddRepo={onAddRepo} />}
        {activeTab === 'scheduler' && <SchedulerTab />}
        {activeTab === 'analytics' && <AnalyticsTab repoId={activeRepoId ?? null} />}
        {activeTab === 'app' && <AppTab onTimezoneChange={onTimezoneChange} />}
      </div>
    </div>
  );
}
