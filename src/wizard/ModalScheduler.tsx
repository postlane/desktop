// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';
import SchedulerConnect from '../settings/SchedulerConnect';
import { ZernioLogo, UploadPostLogo } from '../assets/logos';

type Provider = 'zernio' | 'upload_post';

const VALID_PROVIDERS: readonly Provider[] = ['zernio', 'upload_post'];
function isProvider(p: string): p is Provider {
  return (VALID_PROVIDERS as readonly string[]).includes(p);
}

interface PickerProps {
  onSelect: (p: Provider) => void;
  connected: Provider[];
  workspaceName: string;
}

function ProviderPicker({ onSelect, connected, workspaceName }: PickerProps) {
  return (
    <>
      {workspaceName && (
        <p className="is-size-6 has-text-grey mb-3">
          Workspace: {workspaceName}
        </p>
      )}
      <div className="is-flex mb-4" style={{ gap: 12, maxWidth: 425 }}>
        <button
          className="button"
          style={{ flex: '1 1 0', background: 'white', color: '#1a1a1a', border: '1px solid #e0e0e0' }}
          onClick={() => onSelect('zernio')}
        >
          <ZernioLogo size={16} style={{ marginRight: 8 }} />
          <span>Zernio</span>
          {connected.includes('zernio')
            ? <span className="tag is-success is-light is-small ml-2">Connected</span>
            : <span className="tag is-light is-small ml-2">Recommended</span>}
        </button>
        <button
          className="button"
          style={{ flex: '1 1 0', background: 'white', color: '#1a1a1a', border: '1px solid #e0e0e0' }}
          onClick={() => onSelect('upload_post')}
        >
          <UploadPostLogo size={16} style={{ marginRight: 8 }} />
          <span>Upload Post</span>
          {connected.includes('upload_post')
            ? <span className="tag is-success is-light is-small ml-2">Connected</span>
            : <span className="tag is-light is-small ml-2">10 free</span>}
        </button>
      </div>
      <p className="is-size-7 has-text-grey">
        Scheduler settings are configured per workspace.{' '}
        <a
          href="https://docs.postlane.dev/scheduling"
          className="has-text-link"
          onClick={(e) => { e.preventDefault(); openUrl('https://docs.postlane.dev/scheduling').catch(console.error); }}
        >
          Scheduler setup docs →
        </a>
      </p>
    </>
  );
}

interface Props {
  workspaceId: string;
  workspaceName: string;
  onNext: () => void;
  onBack: () => void;
  setSchedulerLinked: (linked: boolean) => void;
  onSkipToApp?: () => void;
}

export default function ModalScheduler({ workspaceId, workspaceName, onNext, onBack, setSchedulerLinked, onSkipToApp }: Props) {
  const [selectedProvider, setSelectedProvider] = useState<Provider | null>(null);
  const [connectedProviders, setConnectedProviders] = useState<Provider[]>([]);

  useEffect(() => {
    invoke<string[]>('list_connected_providers', { repoId: null })
      .then((providers) => {
        if (Array.isArray(providers)) setConnectedProviders(providers.filter(isProvider));
      })
      .catch(() => {});
  }, []);

  function handleSuccess(provider: string) {
    if (!isProvider(provider)) return;
    setConnectedProviders((prev) => [...prev, provider]);
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
      totalSteps={7}
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
        <ProviderPicker onSelect={setSelectedProvider} connected={connectedProviders} workspaceName={workspaceName} />
      )}
    </WizardShell>
  );
}
