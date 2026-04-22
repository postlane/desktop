// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { Button } from '../components/catalyst/button';
import ReposTab from './ReposTab';
import SchedulerTab from './SchedulerTab';
import AppTab from './AppTab';

type Tab = 'repos' | 'scheduler' | 'app';

interface Props {
  onClose: () => void;
  onTimezoneChange?: (_tz: string) => void;
  onRepoChange?: () => void;
}

export default function SettingsPanel({ onClose, onTimezoneChange, onRepoChange }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>('repos');

  return (
    <div className="flex h-full flex-col bg-white dark:bg-zinc-900">
      <div className="flex items-center justify-between border-b border-zinc-200 px-6 py-4 dark:border-zinc-700">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">Settings</h1>
        <Button plain onClick={onClose} aria-label="Close settings">✕</Button>
      </div>
      <div className="flex border-b border-zinc-200 px-6 dark:border-zinc-700" role="tablist">
        {(['repos', 'scheduler', 'app'] as Tab[]).map((tab) => (
          <button
            key={tab}
            role="tab"
            aria-selected={activeTab === tab}
            onClick={() => setActiveTab(tab)}
            className={[
              'px-4 py-3 text-sm font-medium border-b-2 -mb-px capitalize focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
              activeTab === tab
                ? 'border-zinc-900 text-zinc-900 dark:border-zinc-100 dark:text-zinc-100'
                : 'border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300',
            ].join(' ')}
          >
            {tab}
          </button>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto p-6">
        {activeTab === 'repos' && <ReposTab onRepoChange={() => onRepoChange?.()} />}
        {activeTab === 'scheduler' && <SchedulerTab />}
        {activeTab === 'app' && <AppTab onTimezoneChange={onTimezoneChange} />}
      </div>
    </div>
  );
}
