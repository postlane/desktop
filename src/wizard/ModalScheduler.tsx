// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import WizardShell from './WizardShell';
import SchedulerConnect from '../settings/SchedulerConnect';

type Provider = 'zernio' | 'publer';

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
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <button
            className="button is-light is-fullwidth"
            onClick={() => setSelectedProvider('zernio')}
          >
            <span className="is-flex is-align-items-center" style={{ gap: 8 }}>
              Zernio
              <span className="tag is-primary is-light is-small">Recommended</span>
            </span>
          </button>
          <button
            className="button is-light is-fullwidth"
            onClick={() => setSelectedProvider('publer')}
          >
            Publer
          </button>
        </div>
      )}
    </WizardShell>
  );
}
