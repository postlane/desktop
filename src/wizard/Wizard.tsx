// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useWizardState } from './useWizardState';
import ModalWelcome from './ModalWelcome';
import ModalAccount from './ModalAccount';
import ModalOrgPicker from './ModalOrgPicker';
import ModalScheduler from './ModalScheduler';
import ModalGitHubApp from './ModalGitHubApp';
import ModalProjectContext from './ModalProjectContext';
import ModalComplete from './ModalComplete';
import ModalPricingGate from './ModalPricingGate';

interface Props {
  onComplete: () => void;
  startAt?: number;
}

interface LateStepProps {
  step: number;
  provider: string;
  workspaceId: string;
  workspaceName: string;
  schedulerLinked: boolean;
  onNext: () => void;
  onBack: () => void;
  onComplete: () => void;
}

function WizardLateSteps({ step, provider, workspaceId, workspaceName, schedulerLinked, onNext, onBack, onComplete }: LateStepProps) {
  if (step === 5) {
    return <ModalGitHubApp provider={provider} workspaceId={workspaceId} workspaceName={workspaceName} onNext={onNext} onBack={onBack} />;
  }
  if (step === 6) {
    return <ModalProjectContext workspaceId={workspaceId} workspaceName={workspaceName} onNext={onNext} onBack={onBack} />;
  }
  return <ModalComplete schedulerLinked={schedulerLinked} onComplete={onComplete} onBack={onBack} />;
}

export default function Wizard({ onComplete, startAt }: Props) {
  const wizard = useWizardState({ startAt });
  const [showPricingGate, setShowPricingGate] = useState(false);

  useEffect(() => {
    if (wizard.step > 1) {
      invoke('write_wizard_state', { step: wizard.step }).catch(console.warn);
    }
  }, [wizard.step]);

  const handleSkipToApp = async () => {
    invoke('clear_wizard_state').catch(console.warn);
    try { await invoke('set_wizard_completed'); } catch (e) { console.warn('[wizard] set_wizard_completed failed:', e); }
    onComplete();
  };
  const closePricingGate = () => setShowPricingGate(false);
  const handlePricingSkip = (id: string, name: string) => { wizard.setWorkspaceId(id); wizard.setWorkspaceName(name); setShowPricingGate(false); wizard.next(); };

  const provider = wizard.provider ?? 'github';
  const workspaceId = wizard.workspaceId ?? '';
  const workspaceName = wizard.workspaceName ?? '';

  if (showPricingGate) return <ModalPricingGate onPaid={closePricingGate} onBack={closePricingGate} onSkip={handlePricingSkip} />;
  if (wizard.step === 1) return <ModalWelcome onNext={wizard.next} />;
  if (wizard.step === 2) {
    return <ModalAccount mode={startAt === 2 ? 'add_org' : 'sign_in'} onNext={(p) => { wizard.setToken('detected'); wizard.setProvider(p); wizard.next(); }} onBack={wizard.back} />;
  }
  if (wizard.step === 3) {
    return <ModalOrgPicker onNext={(wid, wname) => { wizard.setWorkspaceId(wid); wizard.setWorkspaceName(wname); wizard.next(); }} onBack={wizard.back} onPricingGate={() => setShowPricingGate(true)} onSkipToApp={handleSkipToApp} provider={provider} />;
  }
  if (wizard.step === 4) {
    return <ModalScheduler workspaceId={workspaceId} workspaceName={workspaceName} onNext={wizard.next} onBack={wizard.back} setSchedulerLinked={wizard.setSchedulerLinked} onSkipToApp={handleSkipToApp} />;
  }
  return <WizardLateSteps step={wizard.step} provider={provider} workspaceId={workspaceId} workspaceName={workspaceName} schedulerLinked={wizard.schedulerLinked} onNext={wizard.next} onBack={wizard.back} onComplete={onComplete} />;
}
