// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
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

export default function ModalWorkspace({ onNext, onBack, onPricingGate }: Props) {
  const [name, setName] = useState('');
  const [workspaceType, setWorkspaceType] = useState<WorkspaceType>('personal');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleNext() {
    setError(null);
    setLoading(true);
    try {
      const result = await invoke<CreateProjectResult>('create_project', { name, workspaceType });
      onNext(result.project_id);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes('no_free_slot')) { onPricingGate(); }
      else { setError(`Failed to create workspace: ${msg}`); }
    } finally {
      setLoading(false);
    }
  }

  return (
    <WizardShell step={3} totalSteps={5} title="Name your workspace"
      subtitle="A workspace is a brand, org, or client — with its own scheduler and writing voice."
      onNext={handleNext} onBack={onBack} nextDisabled={name.trim().length === 0 || loading}
    >
      <WorkspaceForm name={name} workspaceType={workspaceType} error={error}
        onNameChange={setName} onTypeChange={setWorkspaceType} />
    </WizardShell>
  );
}
