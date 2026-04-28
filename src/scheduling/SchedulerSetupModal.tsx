// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  repoName: string;
  repoId?: string;
  onSetupLater: () => void;
  onOpenSchedulerSettings?: (_provider: string) => void;
  onDone?: () => void;
}

const PROVIDERS = [
  { name: 'Zernio', key: 'zernio', description: 'Multi-platform scheduler — X, Bluesky, LinkedIn, Mastodon', freeTier: 'Free tier limit tracked by 429 responses' },
  { name: 'Buffer', key: 'buffer', description: 'Social media scheduling — 3 channels', freeTier: 'Free tier limit tracked by 429 responses' },
  { name: 'Ayrshare', key: 'ayrshare', description: 'Developer-focused multi-platform API', freeTier: 'No free tier — paid plans only' },
  { name: 'Publer', key: 'publer', description: 'Social media scheduler — all major platforms', freeTier: '10 posts/month on free plan' },
  { name: 'Outstand', key: 'outstand', description: 'Pay-as-you-go scheduler — $0.01 per post after free tier', freeTier: '1,000 posts/month free' },
  { name: 'Substack Notes', key: 'substack_notes', description: 'Posts directly to your Notes feed on Substack', freeTier: 'No scheduler queue — posts immediately' },
  { name: 'Webhook', key: 'webhook', description: 'Connect Zapier or Make for custom automation', freeTier: 'Zapier: 100 tasks/month free' },
] as const;

function ModalFooter({ selected, configured, checking, onCheck, onDone, onSetupLater }: {
  selected: string | null; configured: boolean; checking: boolean;
  onCheck: () => void; onDone: () => void; onSetupLater: () => void;
}) {
  return (
    <div className="mt-4 flex justify-end gap-2">
      {selected && !configured && (
        <button type="button" onClick={onCheck} disabled={checking}
          className="rounded-md px-3 py-1.5 text-sm text-blue-600 hover:bg-blue-50 dark:text-blue-400 dark:hover:bg-blue-950">
          {checking ? 'Checking…' : 'Check'}
        </button>
      )}
      {configured ? (
        <button type="button" onClick={onDone}
          className="rounded-md bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700">
          Done
        </button>
      ) : (
        <button type="button" onClick={onSetupLater}
          className="rounded-md px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800">
          Set up later
        </button>
      )}
    </div>
  );
}

export default function SchedulerSetupModal({ repoName, repoId, onSetupLater, onOpenSchedulerSettings, onDone }: Props) {
  const [selected, setSelected] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [configured, setConfigured] = useState(false);

  const handleSelect = useCallback((key: string) => {
    setSelected(key);
    onOpenSchedulerSettings?.(key);
  }, [onOpenSchedulerSettings]);

  const handleCheck = useCallback(async () => {
    if (!repoId) return;
    setChecking(true);
    try { setConfigured(await invoke<boolean>('has_scheduler_configured', { repoId })); }
    catch { /* non-critical */ }
    finally { setChecking(false); }
  }, [repoId]);

  const handleDone = useCallback(async () => {
    try { await invoke('update_scheduler_config', { repoId, primaryProvider: selected }); }
    catch { /* non-critical — provider already in keyring */ }
    onDone?.();
  }, [repoId, selected, onDone]);

  return (
    <div role="dialog" aria-modal="true" aria-labelledby="scheduler-setup-title" className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-zinc-900">
        <h2 id="scheduler-setup-title" className="text-base font-semibold text-zinc-900 dark:text-zinc-100">
          Set up posting for {repoName}
        </h2>
        <p className="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
          Choose a scheduler to enable posting. You can configure multiple providers for automatic fallback.
        </p>
        <ul className="mt-4 divide-y divide-zinc-100 dark:divide-zinc-800">
          {PROVIDERS.map((p) => (
            <li key={p.key} className="flex items-start justify-between py-3">
              <div>
                <p className="text-sm font-medium text-zinc-900 dark:text-zinc-100">{p.name}</p>
                <p className="text-xs text-zinc-500 dark:text-zinc-400">{p.description}</p>
                <p className="text-xs text-zinc-400 dark:text-zinc-500">{p.freeTier}</p>
              </div>
              {onOpenSchedulerSettings && (
                <button type="button" aria-label={`Set up ${p.name}`} onClick={() => handleSelect(p.key)}
                  className="ml-4 shrink-0 text-xs text-blue-600 hover:underline dark:text-blue-400">
                  Set up →
                </button>
              )}
            </li>
          ))}
        </ul>
        {configured && (
          <p className="mt-4 text-sm text-green-600 dark:text-green-400">Scheduler configured. You can now post from this repo.</p>
        )}
        <ModalFooter selected={selected} configured={configured} checking={checking}
          onCheck={handleCheck} onDone={handleDone} onSetupLater={onSetupLater} />
      </div>
    </div>
  );
}
