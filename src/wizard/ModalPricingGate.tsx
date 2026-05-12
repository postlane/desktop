// SPDX-License-Identifier: BUSL-1.1

import { useRef, useState } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

interface Props {
  onPaid: () => void;
  onBack: () => void;
  onSkip?: (projectId: string) => void;
  pollIntervalMs?: number;
  maxAttempts?: number;
}

function useBillingPoller(onPaid: () => void, pollIntervalMs: number, maxAttempts: number) {
  const [polling, setPolling] = useState(false);
  const [timedOut, setTimedOut] = useState(false);
  const attemptsRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopPolling() {
    if (intervalRef.current) { clearInterval(intervalRef.current); intervalRef.current = null; }
  }

  function begin() {
    setTimedOut(false);
    setPolling(true);
    attemptsRef.current = 0;
    intervalRef.current = setInterval(async () => {
      attemptsRef.current++;
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

  return { polling, timedOut, begin, stopPolling };
}

export default function ModalPricingGate({
  onPaid, onBack, onSkip, pollIntervalMs = 5000, maxAttempts = 120,
}: Props) {
  const { polling, timedOut, begin, stopPolling } = useBillingPoller(onPaid, pollIntervalMs, maxAttempts);

  async function handleSubscribe() {
    try { await openUrl('https://postlane.dev/billing'); } catch { /* ignore */ }
    begin();
  }

  async function handleSkip() {
    if (!onSkip) return;
    try {
      const projects = await invoke<{ id: string }[]>('list_projects');
      const first = projects[0];
      if (first) onSkip(first.id);
    } catch { /* non-fatal: skip button disappears if list_projects fails */ }
  }

  return (
    <WizardShell
      step={3}
      totalSteps={5}
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
        {timedOut && (
          <button className="button is-light is-small" onClick={begin}>
            Check again
          </button>
        )}
      </div>
    </WizardShell>
  );
}
