// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '../ipc/invoke';
import { useAsyncCommand } from '../hooks/useAsyncCommand';
import WizardShell from './WizardShell';

type WorkspaceType = 'personal' | 'organization' | 'client';

interface CreateProjectResult {
  project_id: string;
  name: string;
  workspace_type: string;
}

interface Props {
  onNext: (workspaceId: string) => void;
  onBack: () => void;
  onPricingGate: () => void;
  onSkipToApp?: () => void;
}

function toWorkspaceType(v: string): WorkspaceType {
  if (v === 'organization' || v === 'client') return v;
  return 'personal';
}

interface FormProps {
  name: string;
  workspaceType: WorkspaceType;
  error: string | null;
  onNameChange: (v: string) => void;
  onTypeChange: (v: WorkspaceType) => void;
}

function WorkspaceForm({ name, workspaceType, error, onNameChange, onTypeChange }: FormProps) {
  return (
    <div className="mb-4" style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      {error && <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7">{error}</div>}
      <div className="field">
        <label className="label is-small">Workspace name</label>
        <div className="control">
          <input className="input is-small" type="text" placeholder="e.g. Postlane, Acme Corp, Personal"
            value={name} onChange={(e) => onNameChange(e.target.value)} maxLength={100} />
        </div>
      </div>
      <div className="field">
        <label className="label is-small">Workspace type</label>
        <div className="control">
          <div className="select is-small is-fullwidth">
            <select value={workspaceType} onChange={(e) => onTypeChange(toWorkspaceType(e.target.value))}>
              <option value="personal">Personal</option>
              <option value="organization">Organization</option>
              <option value="client">Client project</option>
            </select>
          </div>
        </div>
      </div>
      <p className="is-size-7 has-text-grey">Your workspace name is visible only to you.</p>
    </div>
  );
}

export default function ModalWorkspace({ onNext, onBack, onPricingGate, onSkipToApp }: Props) {
  const [name, setName] = useState('');
  const [workspaceType, setWorkspaceType] = useState<WorkspaceType>('personal');
  const { loading, error, run } = useAsyncCommand();

  async function handleNext() {
    let pricingGate = false;
    const result = await run(async () => {
      try {
        return await invoke<CreateProjectResult>('create_project', { name: name.trim(), workspaceType });
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes('No free project slot')) pricingGate = true;
        throw err;
      }
    });
    if (result !== null) {
      onNext(result.project_id);
    } else if (pricingGate) {
      onPricingGate();
    }
  }

  return (
    <WizardShell step={3} totalSteps={5} title="Name your workspace"
      subtitle="A workspace is a brand, org, or client — with its own scheduler and writing voice."
      onNext={handleNext} onBack={onBack} nextDisabled={name.trim().length === 0 || loading}
      onSkip={onSkipToApp}
    >
      <WorkspaceForm name={name} workspaceType={workspaceType} error={error}
        onNameChange={setName} onTypeChange={setWorkspaceType} />
    </WizardShell>
  );
}
