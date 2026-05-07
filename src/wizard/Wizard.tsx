// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { useWizardState } from './useWizardState';
import ModalWelcome from './ModalWelcome';
import ModalAccount from './ModalAccount';
import ModalWorkspace from './ModalWorkspace';
import ModalScheduler from './ModalScheduler';
import ModalRepository from './ModalRepository';
import ModalPricingGate from './ModalPricingGate';

interface Props {
  onComplete: () => void;
  startAt?: number;
}

export default function Wizard({ onComplete, startAt }: Props) {
  const wizard = useWizardState({ startAt });
  const [showPricingGate, setShowPricingGate] = useState(false);

  if (showPricingGate) {
    return (
      <ModalPricingGate
        onPaid={() => setShowPricingGate(false)}
        onBack={() => setShowPricingGate(false)}
      />
    );
  }

  if (wizard.step === 1) {
    return <ModalWelcome onNext={wizard.next} />;
  }

  if (wizard.step === 2) {
    return (
      <ModalAccount
        onNext={() => { wizard.setToken('detected'); wizard.next(); }}
        onBack={wizard.back}
      />
    );
  }

  if (wizard.step === 3) {
    return (
      <ModalWorkspace
        onNext={(workspaceId) => { wizard.setWorkspaceId(workspaceId); wizard.next(); }}
        onBack={wizard.back}
        onPricingGate={() => setShowPricingGate(true)}
      />
    );
  }

  if (wizard.step === 4) {
    return (
      <ModalScheduler
        workspaceId={wizard.workspaceId ?? ''}
        onNext={wizard.next}
        onBack={wizard.back}
        setSchedulerLinked={wizard.setSchedulerLinked}
      />
    );
  }

  return (
    <ModalRepository
      workspaceId={wizard.workspaceId ?? ''}
      onBack={wizard.back}
      onComplete={onComplete}
    />
  );
}
