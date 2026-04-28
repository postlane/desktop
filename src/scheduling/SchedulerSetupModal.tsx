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

type ProviderEntry = typeof PROVIDERS[number];

function ProviderRow({ p, priority, isPending, isChecking, onSetUp, onCheck, onRemove }: {
  p: ProviderEntry; priority: number; isPending: boolean; isChecking: boolean;
  onSetUp?: () => void; onCheck: () => void; onRemove: () => void;
}) {
  return (
    <li className="flex items-start justify-between py-3">
      <div>
        {priority > 0 && <span className="mr-1.5 text-xs font-bold text-blue-600 dark:text-blue-400">#{priority}</span>}
        <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">{p.name}</span>
        <p className="text-xs text-zinc-500 dark:text-zinc-400">{p.description}</p>
        <p className="text-xs text-zinc-400 dark:text-zinc-500">{p.freeTier}</p>
      </div>
      <div className="ml-4 flex shrink-0 items-center gap-2">
        {priority > 0 ? (
          <>
            <span className="text-xs text-green-600 dark:text-green-400">Configured ✓</span>
            <button type="button" aria-label={`Remove ${p.name}`} onClick={onRemove}
              className="text-xs text-zinc-400 hover:text-red-600 dark:hover:text-red-400">Remove</button>
          </>
        ) : (
          <>
            {isPending && (
              <button type="button" aria-label={`Check ${p.name} configured`} onClick={onCheck} disabled={isChecking}
                className="text-xs text-blue-600 hover:underline dark:text-blue-400">
                {isChecking ? 'Checking…' : 'Check ✓'}
              </button>
            )}
            {onSetUp && (
              <button type="button" aria-label={`Set up ${p.name}`} onClick={onSetUp}
                className="text-xs text-blue-600 hover:underline dark:text-blue-400">
                Set up →
              </button>
            )}
          </>
        )}
      </div>
    </li>
  );
}

function ModalFooter({ hasConfigured, onDone, onSetupLater }: {
  hasConfigured: boolean; onDone: () => void; onSetupLater: () => void;
}) {
  return (
    <div className="mt-4 flex justify-end gap-2">
      <button type="button" onClick={onSetupLater}
        className="rounded-md px-3 py-1.5 text-sm text-zinc-600 hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800">
        Set up later
      </button>
      {hasConfigured && (
        <button type="button" onClick={onDone}
          className="rounded-md bg-blue-600 px-3 py-1.5 text-sm text-white hover:bg-blue-700">
          Done
        </button>
      )}
    </div>
  );
}

export default function SchedulerSetupModal({ repoName, repoId, onSetupLater, onOpenSchedulerSettings, onDone }: Props) {
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [ordered, setOrdered] = useState<string[]>([]);
  const [checking, setChecking] = useState<string | null>(null);

  const handleSetUp = useCallback((key: string) => {
    setPending((prev) => new Set([...prev, key]));
    onOpenSchedulerSettings?.(key);
  }, [onOpenSchedulerSettings]);

  const handleCheck = useCallback(async (key: string) => {
    if (!repoId) return;
    setChecking(key);
    try {
      const ok = await invoke<boolean>('has_provider_credential', { repoId, provider: key });
      if (ok) {
        setOrdered((prev) => prev.includes(key) ? prev : [...prev, key]);
        setPending((prev) => { const next = new Set(prev); next.delete(key); return next; });
      }
    } catch { /* non-critical */ }
    finally { setChecking(null); }
  }, [repoId]);

  const handleRemove = useCallback((key: string) => {
    setOrdered((prev) => prev.filter((p) => p !== key));
  }, []);

  const handleDone = useCallback(async () => {
    try { await invoke('update_scheduler_config', { repoId, fallbackOrder: ordered }); }
    catch { /* non-critical — config may already be written */ }
    onDone?.();
  }, [repoId, ordered, onDone]);

  return (
    <div role="dialog" aria-modal="true" aria-labelledby="scheduler-setup-title" className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="w-full max-w-lg rounded-xl bg-white p-6 shadow-xl dark:bg-zinc-900">
        <h2 id="scheduler-setup-title" className="text-base font-semibold text-zinc-900 dark:text-zinc-100">
          Set up posting for {repoName}
        </h2>
        <p className="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
          Add providers in priority order. When one reaches its free tier, the next is used automatically.
        </p>
        <ul className="mt-4 divide-y divide-zinc-100 dark:divide-zinc-800">
          {PROVIDERS.map((p) => (
            <ProviderRow key={p.key} p={p}
              priority={ordered.indexOf(p.key) + 1}
              isPending={pending.has(p.key)}
              isChecking={checking === p.key}
              onSetUp={onOpenSchedulerSettings ? () => handleSetUp(p.key) : undefined}
              onCheck={() => handleCheck(p.key)}
              onRemove={() => handleRemove(p.key)}
            />
          ))}
        </ul>
        {ordered.length > 0 && (
          <p className="mt-3 text-sm text-green-600 dark:text-green-400">
            {ordered.length === 1 ? 'Scheduler configured.' : `${ordered.length} providers configured — automatic fallback active.`}
          </p>
        )}
        <ModalFooter hasConfigured={ordered.length > 0} onDone={handleDone} onSetupLater={onSetupLater} />
      </div>
    </div>
  );
}
