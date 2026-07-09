// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4 -- orchestrates the 6-step workspace setup flow with a
// Mantine Stepper, accumulating each step's patch into one WorkspaceConfig
// draft. Replaces Wizard.tsx's back half (ModalScheduler/ModalGitHubApp/
// ModalProjectContext/ModalComplete) once an org/project context exists.
//
// Known gap: only the aggregate config survives step navigation -- each
// step's own local UI state resets if you go Back and then Forward again
// past it. The design brief calls for full value preservation on back/
// forward; this covers the data (nothing is lost from the final submit),
// not yet the redisplayed UI state on every step.

import { useState } from 'react';
import { Stepper, Title } from '@mantine/core';
import StepFolderPick from './StepFolderPick';
import StepBasicConfig, { type BasicConfigPatch } from './StepBasicConfig';
import StepLlm, { type LlmPatch } from './StepLlm';
import StepScheduler, { type SchedulerPatch } from './StepScheduler';
import StepAttribution, { type AttributionPatch } from './StepAttribution';
import StepReview from './StepReview';
import type { ChildRepo, WorkspaceConfig } from './types';

interface Props {
  projectId: string;
  projectName: string;
  onComplete: () => void;
  onBack: () => void;
  /** Called when the user clicks "Add to plan" on a paid_required success
   *  banner (Step 6) -- the caller decides where that goes (Settings ->
   *  Account, 24.4.9), since this component has no navigation of its own. */
  onUpgradeClick?: () => void;
}

type ConfigDraft = Partial<Omit<WorkspaceConfig, 'project_id'>>;

const STEP_LABELS = ['Folder', 'Basic config', 'LLM', 'Scheduler', 'Attribution', 'Review'];

export default function WorkspaceSetupWizard({ projectId, projectName, onComplete, onBack, onUpgradeClick }: Props) {
  const [step, setStep] = useState(0);
  const [workspacePath, setWorkspacePath] = useState('');
  const [childRepos, setChildRepos] = useState<ChildRepo[]>([]);
  const [draft, setDraft] = useState<ConfigDraft>({});

  function handleFolderNext(path: string, repos: ChildRepo[]) {
    setWorkspacePath(path);
    setChildRepos(repos);
    setStep(1);
  }
  function handleBasicConfigNext(patch: BasicConfigPatch) {
    setDraft((prev) => ({ ...prev, ...patch }));
    setStep(2);
  }
  function handleLlmNext(patch: LlmPatch) {
    setDraft((prev) => ({ ...prev, ...patch }));
    setStep(3);
  }
  function handleSchedulerNext(patch: SchedulerPatch) {
    setDraft((prev) => ({ ...prev, ...patch }));
    setStep(4);
  }
  function handleAttributionNext(patch: AttributionPatch) {
    setDraft((prev) => ({ ...prev, ...patch }));
    setStep(5);
  }

  return (
    <div>
      {projectName && <Title order={4} mb="sm">Set up &quot;{projectName}&quot;</Title>}
      <Stepper active={step} allowNextStepsSelect={false} mb="md">
        {STEP_LABELS.map((label) => <Stepper.Step key={label} label={label} />)}
      </Stepper>
      {step === 0 && <StepFolderPick onNext={handleFolderNext} onBack={onBack} />}
      {step === 1 && <StepBasicConfig onNext={handleBasicConfigNext} onBack={() => setStep(0)} />}
      {step === 2 && <StepLlm onNext={handleLlmNext} onBack={() => setStep(1)} />}
      {step === 3 && <StepScheduler onNext={handleSchedulerNext} onBack={() => setStep(2)} />}
      {step === 4 && <StepAttribution onNext={handleAttributionNext} onBack={() => setStep(3)} />}
      {step === 5 && (
        <StepReview
          workspacePath={workspacePath}
          childRepos={childRepos}
          config={{ project_id: projectId, ...draft } as WorkspaceConfig}
          onBack={() => setStep(4)}
          onComplete={onComplete}
          onUpgradeClick={onUpgradeClick ?? (() => {})}
        />
      )}
    </div>
  );
}
