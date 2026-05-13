// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useWizardState } from './useWizardState';
import ModalWelcome from './ModalWelcome';
import ModalAccount from './ModalAccount';
import ModalOrgPicker from './ModalOrgPicker';
import ModalScheduler from './ModalScheduler';
import ModalGitHubApp from './ModalGitHubApp';
import ModalPricingGate from './ModalPricingGate';

interface Props {
  onComplete: () => void;
  startAt?: number;
}

export default function Wizard({ onComplete, startAt }: Props) {
  const wizard = useWizardState({ startAt });
  const [showPricingGate, setShowPricingGate] = useState(false);

  const handleSkipToApp = async () => { try { await invoke('set_wizard_completed'); } catch { /* non-fatal */ } onComplete(); };
  const closePricingGate = () => setShowPricingGate(false);
  const handlePricingSkip = (id: string, name: string) => { wizard.setWorkspaceId(id); wizard.setWorkspaceName(name); setShowPricingGate(false); wizard.next(); };

  const provider = wizard.provider ?? 'github';
  const workspaceId = wizard.workspaceId ?? '';
  const workspaceName = wizard.workspaceName ?? '';

  if (showPricingGate) return <ModalPricingGate onPaid={closePricingGate} onBack={closePricingGate} onSkip={handlePricingSkip} />;

  if (wizard.step === 1) {
    return <ModalWelcome onNext={wizard.next} />;
  }

  if (wizard.step === 2) {
    return (
      <ModalAccount
        mode={startAt === 2 ? 'add_org' : 'sign_in'}
        onNext={(p) => { wizard.setToken('detected'); wizard.setProvider(p); wizard.next(); }}
        onBack={wizard.back}
      />
    );
  }

  if (wizard.step === 3) {
    return (
      <ModalOrgPicker
        onNext={(wid, wname) => { wizard.setWorkspaceId(wid); wizard.setWorkspaceName(wname); wizard.next(); }}
        onBack={wizard.back}
        onPricingGate={() => setShowPricingGate(true)}
        onSkipToApp={handleSkipToApp}
        provider={provider}
      />
    );
  }

  if (wizard.step === 4) {
    return (
      <ModalScheduler
        workspaceId={workspaceId}
        workspaceName={workspaceName}
        onNext={wizard.next}
        onBack={wizard.back}
        setSchedulerLinked={wizard.setSchedulerLinked}
        onSkipToApp={handleSkipToApp}
      />
    );
  }

  return (
    <ModalGitHubApp
      provider={provider}
      workspaceId={workspaceId}
      workspaceName={workspaceName}
      onNext={handleSkipToApp}
      onBack={wizard.back}
    />
  );
}
