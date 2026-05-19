// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
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
import ModalProviderLinked from './ModalProviderLinked';

interface Props {
  onComplete: () => void;
  startAt?: number;
  initialProvider?: string | null;
  initialWorkspaceId?: string | null;
  initialWorkspaceName?: string | null;
}

interface LateStepProps {
  step: number;
  provider: string;
  workspaceId: string;
  workspaceName: string;
  schedulerLinked: boolean;
  repoConnected: boolean;
  setRepoConnected: (_v: boolean) => void;
  onNext: () => void;
  onBack: () => void;
  onComplete: () => void;
}

interface Step3Props {
  provider: string;
  showProviderLinked: boolean;
  linkedProviders: string[];
  onContinue: () => void;
  onOrgNext: (wid: string, wname: string) => void;
  onBack: () => void;
  onPricingGate: () => void;
  onSkipToApp: () => void;
}

function WizardStep3({ provider, showProviderLinked, linkedProviders, onContinue, onOrgNext, onBack, onPricingGate, onSkipToApp }: Step3Props) {
  if (showProviderLinked) {
    return <ModalProviderLinked currentProvider={provider} linkedProviders={linkedProviders} onContinue={onContinue} />;
  }
  return <ModalOrgPicker onNext={onOrgNext} onBack={onBack} onPricingGate={onPricingGate} onSkipToApp={onSkipToApp} provider={provider} />;
}

function WizardLateSteps({ step, provider, workspaceId, workspaceName, schedulerLinked, repoConnected, setRepoConnected, onNext, onBack, onComplete }: LateStepProps) {
  if (step === 5) {
    return <ModalGitHubApp provider={provider} workspaceId={workspaceId} workspaceName={workspaceName} onNext={onNext} onBack={onBack} setRepoConnected={setRepoConnected} />;
  }
  if (step === 6) {
    return <ModalProjectContext workspaceId={workspaceId} workspaceName={workspaceName} onNext={onNext} onBack={onBack} />;
  }
  return <ModalComplete schedulerLinked={schedulerLinked} repoConnected={repoConnected} onComplete={onComplete} onBack={onBack} />;
}

export default function Wizard({ onComplete, startAt, initialProvider, initialWorkspaceId, initialWorkspaceName }: Props) {
  const wizard = useWizardState({
    startAt,
    initialProvider,
    initialWorkspaceId,
    initialWorkspaceName,
  });
  const [showPricingGate, setShowPricingGate] = useState(false);
  const [repoConnected, setRepoConnected] = useState(false);
  const [showProviderLinked, setShowProviderLinked] = useState(false);
  const [linkedProviders, setLinkedProviders] = useState<string[]>([]);
  const providerCheckDone = useRef(false);
  const isNewLinkRef = useRef(false);

  useEffect(() => {
    if (wizard.step !== 3 || providerCheckDone.current) return;
    providerCheckDone.current = true;
    if (!isNewLinkRef.current) return;
    invoke<string[]>('list_linked_providers')
      .then((providers) => {
        if (providers.length > 1) {
          setLinkedProviders(providers);
          setShowProviderLinked(true);
        }
      })
      .catch(console.warn);
  }, [wizard.step]);

  useEffect(() => {
    if (wizard.step > 1) {
      invoke('write_wizard_state', {
        step: wizard.step,
        provider: wizard.provider,
        workspaceId: wizard.workspaceId,
        workspaceName: wizard.workspaceName,
      }).catch(console.warn);
    }
  }, [wizard.step, wizard.provider, wizard.workspaceId, wizard.workspaceName]);

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
    return <ModalAccount mode={startAt === 2 ? 'add_org' : 'sign_in'} onNext={(p, newLink) => { wizard.setToken('detected'); wizard.setProvider(p); isNewLinkRef.current = newLink; wizard.next(); }} onBack={wizard.back} />;
  }
  if (wizard.step === 3) {
    return <WizardStep3 provider={provider} showProviderLinked={showProviderLinked} linkedProviders={linkedProviders} onContinue={() => setShowProviderLinked(false)} onOrgNext={(wid, wname) => { wizard.setWorkspaceId(wid); wizard.setWorkspaceName(wname); wizard.next(); }} onBack={wizard.back} onPricingGate={() => setShowPricingGate(true)} onSkipToApp={handleSkipToApp} />;
  }
  if (wizard.step === 4) {
    return <ModalScheduler workspaceId={workspaceId} workspaceName={workspaceName} onNext={wizard.next} onBack={wizard.back} setSchedulerLinked={wizard.setSchedulerLinked} onSkipToApp={wizard.next} />;
  }
  return <WizardLateSteps step={wizard.step} provider={provider} workspaceId={workspaceId} workspaceName={workspaceName} schedulerLinked={wizard.schedulerLinked} repoConnected={repoConnected} setRepoConnected={setRepoConnected} onNext={wizard.next} onBack={wizard.back} onComplete={onComplete} />;
}
