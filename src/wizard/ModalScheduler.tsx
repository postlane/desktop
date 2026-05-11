// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';
import SchedulerConnect from '../settings/SchedulerConnect';
import { ZernioLogo, PublerLogo } from '../assets/logos';

type Provider = 'zernio' | 'publer';

interface PickerProps {
  onSelect: (p: Provider) => void;
  connected: Provider[];
}

function ProviderPicker({ onSelect, connected }: PickerProps) {
  return (
    <>
      <div className="is-flex mb-4" style={{ gap: 12, maxWidth: 425 }}>
        <button
          className="button"
          style={{ flex: '1 1 0', background: 'white', color: '#1a1a1a', border: '1px solid #e0e0e0' }}
          onClick={() => onSelect('zernio')}
          disabled={connected.includes('zernio')}
        >
          <ZernioLogo size={16} style={{ marginRight: 8 }} />
          <span>Zernio</span>
          <span className="tag is-light is-small ml-2">Recommended</span>
        </button>
        <button
          className="button"
          style={{ flex: '1 1 0', background: 'white', color: '#1a1a1a', border: '1px solid #e0e0e0' }}
          onClick={() => onSelect('publer')}
          disabled={connected.includes('publer')}
        >
          <PublerLogo size={16} style={{ marginRight: 8 }} />
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
  onSkipToApp?: () => void;
}

export default function ModalScheduler({ workspaceId, onNext, onBack, setSchedulerLinked, onSkipToApp }: Props) {
  const [selectedProvider, setSelectedProvider] = useState<Provider | null>(null);
  const [connectedProviders, setConnectedProviders] = useState<Provider[]>([]);

  function handleSuccess(provider: string) {
    setConnectedProviders((prev) => [...prev, provider as Provider]);
    setSchedulerLinked(true);
    setSelectedProvider(null);
  }

  function handleSkip() {
    setSchedulerLinked(false);
    onNext();
  }

  function handleBack() {
    if (selectedProvider) {
      setSelectedProvider(null);
    } else {
      onBack();
    }
  }

  const hasConnected = connectedProviders.length > 0;

  return (
    <WizardShell
      step={4}
      totalSteps={5}
      title="Connect a scheduler"
      subtitle="Your scheduler publishes to your social accounts. You bring the key."
      onNext={onNext}
      onBack={handleBack}
      nextHidden={!hasConnected || selectedProvider !== null}
      onSkip={!hasConnected && !selectedProvider ? (onSkipToApp ?? handleSkip) : undefined}
    >
      {selectedProvider ? (
        <SchedulerConnect
          workspaceId={workspaceId}
          provider={selectedProvider}
          onSuccess={handleSuccess}
          onCancel={() => setSelectedProvider(null)}
        />
      ) : (
        <ProviderPicker onSelect={setSelectedProvider} connected={connectedProviders} />
      )}
    </WizardShell>
  );
}
