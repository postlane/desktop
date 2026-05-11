// SPDX-License-Identifier: BUSL-1.1

import { useState, useCallback } from 'react';
import { invoke } from '../ipc/invoke';

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
    <li style={{ borderBottom: '1px solid #ededed', padding: '0.75rem 0', display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between' }}>
      <div>
        {priority > 0 && <span className="tag is-link is-light is-small mr-2">#{priority}</span>}
        <span className="has-text-weight-medium is-size-7">{p.name}</span>
        <p className="is-size-7 has-text-grey">{p.description}</p>
        <p className="is-size-7 has-text-grey-light">{p.freeTier}</p>
      </div>
      <div style={{ marginLeft: '1rem', display: 'flex', alignItems: 'center', gap: '0.5rem', flexShrink: 0 }}>
        {priority > 0 ? (
          <>
            <span className="is-size-7 has-text-success">Configured ✓</span>
            <button type="button" aria-label={`Remove ${p.name}`} onClick={onRemove}
              className="button is-ghost is-small has-text-danger">Remove</button>
          </>
        ) : (
          <>
            {isPending && (
              <button type="button" aria-label={`Check ${p.name} configured`} onClick={onCheck} disabled={isChecking}
                className="button is-ghost is-small has-text-link">
                {isChecking ? 'Checking…' : 'Check ✓'}
              </button>
            )}
            {onSetUp && (
              <button type="button" aria-label={`Set up ${p.name}`} onClick={onSetUp}
                className="button is-ghost is-small has-text-link">Set up →</button>
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
    <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
      <button type="button" className="button is-ghost" onClick={onSetupLater}>Set up later</button>
      {hasConfigured && (
        <button type="button" className="button is-primary" onClick={onDone}>Done</button>
      )}
    </footer>
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

  const handleRemove = useCallback(async (key: string) => {
    try { await invoke('delete_scheduler_credential', { provider: key, repoId: repoId ?? null }); } catch { /* non-critical */ }
    setOrdered((prev) => prev.filter((p) => p !== key));
  }, [repoId]);

  const handleDone = useCallback(async () => {
    try { await invoke('update_scheduler_config', { repoId, fallbackOrder: ordered }); }
    catch { /* non-critical — config may already be written */ }
    onDone?.();
  }, [repoId, ordered, onDone]);

  return (
    <div className="modal is-active">
      <div className="modal-background" />
      <div className="modal-card" role="dialog" aria-modal="true" aria-labelledby="scheduler-setup-title">
        <header className="modal-card-head"><p id="scheduler-setup-title" className="modal-card-title">Set up posting for {repoName}</p></header>
        <section className="modal-card-body">
          <p className="is-size-7 has-text-grey mb-4">Add providers in priority order. When one reaches its free tier, the next is used automatically.</p>
          <ul>
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
            <p className="is-size-7 has-text-success mt-3">
              {ordered.length === 1 ? 'Scheduler configured.' : `${ordered.length} providers configured — automatic fallback active.`}
            </p>
          )}
        </section>
        <ModalFooter hasConfigured={ordered.length > 0} onDone={handleDone} onSetupLater={onSetupLater} />
      </div>
    </div>
  );
}
