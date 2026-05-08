// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';
import SchedulerConnect from '../settings/SchedulerConnect';

type Provider = 'zernio' | 'publer';

function ZernioLogo() {
  return (
    <svg width="16" height="16" viewBox="0 0 32 32" fill="currentColor" aria-hidden="true" style={{ marginRight: 8 }}>
      <path d="M4 6h24v4L10 22h18v4H4v-4L22 10H4V6z" />
    </svg>
  );
}

function PublerLogo() {
  return (
    <svg width="16" height="16" viewBox="0 0 32 32" fill="currentColor" aria-hidden="true" style={{ marginRight: 8 }}>
      <path d="M6 4h12a8 8 0 010 16H10v8H6V4zm4 4v8h8a4 4 0 000-8h-8z" />
    </svg>
  );
}

function ProviderPicker({ onSelect }: { onSelect: (p: Provider) => void }) {
  return (
    <>
      <div className="is-flex mb-4" style={{ gap: 12, maxWidth: 425 }}>
        <button
          className="button is-flex-grow-1"
          style={{ background: '#0052CC', color: 'white', border: 'none' }}
          onClick={() => onSelect('zernio')}
        >
          <ZernioLogo />
          <span>Zernio</span>
          <span className="tag is-light is-small ml-2">Recommended</span>
        </button>
        <button
          className="button is-flex-grow-1"
          style={{ background: '#1B3A5C', color: 'white', border: 'none' }}
          onClick={() => onSelect('publer')}
        >
          <PublerLogo />
          Publer
        </button>
      </div>
      <p className="is-size-7 has-text-grey">
        You can add more schedulers from the dashboard.{' '}
        <a
          href="https://docs.postlane.dev/scheduling"
          className="has-text-link"
          onClick={(e) => { e.preventDefault(); openUrl('https://docs.postlane.dev/scheduling').catch(console.error); }}
        >
          Scheduling docs
        </a>
      </p>
    </>
  );
}

interface Props {
  workspaceId: string;
  onNext: () => void;
  onBack: () => void;
  setSchedulerLinked: (linked: boolean) => void;
}

export default function ModalScheduler({ workspaceId, onNext, onBack, setSchedulerLinked }: Props) {
  const [selectedProvider, setSelectedProvider] = useState<Provider | null>(null);

  function handleSkip() {
    setSchedulerLinked(false);
    onNext();
  }

  function handleSuccess() {
    setSchedulerLinked(true);
    onNext();
  }

  function handleCancel() {
    setSelectedProvider(null);
  }

  function handleBack() {
    if (selectedProvider) {
      setSelectedProvider(null);
    } else {
      onBack();
    }
  }

  return (
    <WizardShell
      step={4}
      totalSteps={5}
      title="Connect a scheduler"
      subtitle="Your scheduler publishes to your social accounts. You bring the key."
      onNext={onNext}
      onBack={handleBack}
      nextHidden
      onSkip={!selectedProvider ? handleSkip : undefined}
    >
      {selectedProvider ? (
        <SchedulerConnect
          workspaceId={workspaceId}
          provider={selectedProvider}
          onSuccess={handleSuccess}
          onCancel={handleCancel}
        />
      ) : (
        <ProviderPicker onSelect={setSelectedProvider} />
      )}
    </WizardShell>
  );
}
