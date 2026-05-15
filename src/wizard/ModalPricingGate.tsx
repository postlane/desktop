// SPDX-License-Identifier: BUSL-1.1

import { useRef, useState } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

interface Props {
  onPaid: () => void;
  onBack: () => void;
  onSkip?: (projectId: string, projectName: string) => void;
  pollIntervalMs?: number;
  maxAttempts?: number;
}

function useBillingPoller(onPaid: () => void, pollIntervalMs: number, maxAttempts: number) {
  const [polling, setPolling] = useState(false);
  const [timedOut, setTimedOut] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const attemptsRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopPolling() {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }

  function begin() {
    setTimedOut(false);
    setPolling(true);
    setAttempts(0);
    attemptsRef.current = 0;
    intervalRef.current = setInterval(async () => {
      attemptsRef.current++;
      setAttempts(attemptsRef.current);
      try {
        const gate = await invoke<string>('check_billing_gate');
        if (gate === 'free' || gate === 'paid') {
          stopPolling();
          setPolling(false);
          onPaid();
          return;
        }
      } catch { /* ignore */ }
      if (attemptsRef.current >= maxAttempts) {
        stopPolling();
        setPolling(false);
        setTimedOut(true);
      }
    }, pollIntervalMs);
  }

  return { polling, timedOut, attempts, begin, stopPolling };
}

const STILL_CHECKING_AFTER_MS = 120_000

function PollingStatus({ polling, timedOut, attempts, pollIntervalMs, onCheckAgain }: {
  polling: boolean; timedOut: boolean; attempts: number; pollIntervalMs: number; onCheckAgain: () => void;
}) {
  const longThreshold = Math.floor(STILL_CHECKING_AFTER_MS / pollIntervalMs)
  if (timedOut) {
    return (
      <>
        <p className="is-size-7 has-text-grey">
          Not detected within 10 minutes — did payment complete? Check your email or click Check again.
        </p>
        <button className="button is-light is-small" onClick={onCheckAgain}>Check again</button>
      </>
    )
  }
  if (polling && attempts >= longThreshold) {
    return <p className="is-size-7 has-text-grey">Still checking...</p>
  }
  return null
}

export default function ModalPricingGate({
  onPaid, onBack, onSkip, pollIntervalMs = 5000, maxAttempts = 120,
}: Props) {
  const { polling, timedOut, attempts, begin, stopPolling } = useBillingPoller(onPaid, pollIntervalMs, maxAttempts);

  async function handleSubscribe() {
    try { await openUrl('https://postlane.dev/billing'); } catch { /* ignore */ }
    begin();
  }

  async function handleSkip() {
    if (!onSkip) return;
    try {
      const projects = await invoke<{ id: string; name: string }[]>('list_projects');
      const first = projects[0];
      if (first) onSkip(first.id, first.name);
    } catch (e) { console.warn('[wizard] list_projects failed, skip aborted:', e); }
  }

  return (
    <WizardShell
      step={3}
      totalSteps={7}
      title="Add a new workspace"
      subtitle="You've used your free workspace. Each additional workspace is $5/month."
      onNext={() => { /* advance handled by polling */ }}
      onBack={() => { stopPolling(); onBack(); }}
      nextHidden
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <p className="is-size-7">
          You can invite collaborators to share one subscription across a team.
        </p>
        <button
          className="button is-primary is-small"
          onClick={handleSubscribe}
          disabled={polling}
        >
          Subscribe — $5/month
        </button>
        {onSkip && (
          <button className="button is-light is-small" onClick={handleSkip}>
            Skip — use existing workspace
          </button>
        )}
        <PollingStatus polling={polling} timedOut={timedOut} attempts={attempts} pollIntervalMs={pollIntervalMs} onCheckAgain={begin} />
      </div>
    </WizardShell>
  );
}
