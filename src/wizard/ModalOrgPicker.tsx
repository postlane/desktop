// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

interface OrgSummary {
  login: string;
  display_name: string;
  avatar_url: string;
  is_personal: boolean;
  has_project: boolean;
  project_id: string | null;
}

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
  provider?: string;
}

interface OrgRowProps {
  org: OrgSummary;
  selected: boolean;
  onSelect: () => void;
}

interface OrgListViewProps {
  orgs: OrgSummary[];
  selectedOrg: OrgSummary | null;
  name: string;
  createError: string | null;
  creating: boolean;
  onBack: () => void;
  onSkipToApp?: () => void;
  onNext: () => void;
  onSelectOrg: (org: OrgSummary) => void;
  onNameChange: (name: string) => void;
}

function OrgRow({ org, selected, onSelect }: OrgRowProps) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      className={`button is-fullwidth is-justify-content-flex-start mb-2 ${selected ? 'is-primary' : 'is-light'}`}
      style={{ height: 'auto', padding: '10px 12px' }}
    >
      <img src={org.avatar_url} alt={org.display_name} width={32} height={32}
        style={{ borderRadius: '50%', marginRight: 10 }} />
      <span style={{ flex: 1, textAlign: 'left' }}>
        <strong>{org.login}</strong>
        {org.is_personal && <span className="tag is-small ml-2">Personal</span>}
      </span>
      {org.has_project && <span className="tag is-light is-small">Existing</span>}
    </button>
  );
}

function ScopeError({ provider, onBack }: { provider: string; onBack: () => void }) {
  function handleReauth() {
    openUrl(`https://postlane.dev/login?desktop=1&provider=${provider}`).catch(console.error);
  }
  return (
    <WizardShell step={3} totalSteps={5}
      title="Permission needed"
      subtitle="Postlane needs permission to read your organisation list."
      onNext={() => {}} onBack={onBack} nextHidden
    >
      <p className="mb-4 is-size-7">Sign in again to grant the <code>read:org</code> scope.</p>
      <button className="button is-primary is-small" onClick={handleReauth}>Sign in again</button>
    </WizardShell>
  );
}

interface LoadErrorProps { message: string; onBack: () => void; onRetry: () => void; }

function LoadError({ message, onBack, onRetry }: LoadErrorProps) {
  return (
    <WizardShell step={3} totalSteps={5}
      title="Could not load organisations"
      subtitle="Check your connection and try again."
      onNext={() => {}} onBack={onBack} nextHidden
    >
      <div role="alert" className="notification is-danger is-light is-size-7 mb-4">{message}</div>
      <button className="button is-light is-small" onClick={onRetry}>Retry</button>
    </WizardShell>
  );
}

function OrgLoadingView({ onBack, onNext }: { onBack: () => void; onNext: () => void }) {
  return (
    <WizardShell step={3} totalSteps={5}
      title="Choose your account or org"
      subtitle="Select the account or organisation this workspace is for."
      onNext={onNext} onBack={onBack} nextDisabled nextHidden
    >
      <p className="has-text-grey is-size-7">Loading organisations…</p>
    </WizardShell>
  );
}

function OrgListView({ orgs, selectedOrg, name, createError, creating, onBack, onSkipToApp, onNext, onSelectOrg, onNameChange }: OrgListViewProps) {
  return (
    <WizardShell step={3} totalSteps={5}
      title="Choose your account or org"
      subtitle="Select the account or organisation this workspace is for."
      onNext={onNext} onBack={onBack}
      nextDisabled={selectedOrg === null || (!selectedOrg.has_project && name.trim().length === 0) || creating}
      onSkip={onSkipToApp}
    >
      <div role="listbox" aria-label="Organisations" className="mb-4">
        {orgs.map((org) => (
          <OrgRow key={org.login} org={org} selected={selectedOrg?.login === org.login} onSelect={() => onSelectOrg(org)} />
        ))}
      </div>
      {selectedOrg !== null && !selectedOrg.has_project && (
        <div className="field">
          <label className="label is-small">Workspace name</label>
          <div className="control">
            <input className="input is-small" type="text" value={name}
              onChange={(e) => onNameChange(e.target.value)} maxLength={100} />
          </div>
        </div>
      )}
      {selectedOrg !== null && selectedOrg.has_project && (
        <p className="is-size-7 has-text-grey">You already have a workspace here. Click Next to open it.</p>
      )}
      {createError !== null && (
        <div role="alert" className="notification is-danger is-light py-2 px-3 is-size-7 mt-3">
          {createError}
        </div>
      )}
    </WizardShell>
  );
}

function useOrgList(provider: string) {
  const [orgs, setOrgs] = useState<OrgSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [scopeError, setScopeError] = useState(false);
  const [retryCount, setRetryCount] = useState(0);

  useEffect(() => {
    setLoadError(null);
    setScopeError(false);
    setOrgs(null);
    invoke<OrgSummary[]>('list_provider_orgs', { provider })
      .then(setOrgs)
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes('scope_not_granted')) { setScopeError(true); } else { setLoadError(msg); }
      });
  }, [provider, retryCount]);

  return { orgs, loadError, scopeError, retry: () => setRetryCount((c) => c + 1) };
}

export default function ModalOrgPicker({ onNext, onBack, onPricingGate, onSkipToApp, provider = 'github' }: Props) {
  const { orgs, loadError, scopeError, retry } = useOrgList(provider);
  const [selectedOrg, setSelectedOrg] = useState<OrgSummary | null>(null);
  const [name, setName] = useState('');
  const [createError, setCreateError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);

  function handleSelectOrg(org: OrgSummary) {
    setSelectedOrg(org);
    setName(org.display_name);
    setCreateError(null);
  }

  async function handleNext() {
    if (!selectedOrg) return;

    // Existing workspace — route directly without creating
    if (selectedOrg.has_project) {
      if (selectedOrg.project_id) onNext(selectedOrg.project_id);
      return;
    }

    if (name.trim().length === 0) return;
    setCreateError(null);
    setCreating(true);
    try {
      const params = {
        name: name.trim(),
        workspaceType: selectedOrg.is_personal ? 'personal' : 'organization',
        ...(selectedOrg.is_personal ? {} : { providerOrgLogin: selectedOrg.login }),
      };
      const result = await invoke<CreateProjectResult>('create_project', params);
      onNext(result.project_id);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes('No free project slot')) {
        onPricingGate();
      } else {
        setCreateError(`Failed to create workspace: ${msg}`);
      }
    } finally {
      setCreating(false);
    }
  }

  if (scopeError) return <ScopeError provider={provider} onBack={onBack} />;
  if (loadError) return <LoadError message={loadError} onBack={onBack} onRetry={retry} />;
  if (orgs === null) return <OrgLoadingView onBack={onBack} onNext={handleNext} />;
  return (
    <OrgListView
      orgs={orgs} selectedOrg={selectedOrg} name={name} createError={createError} creating={creating}
      onBack={onBack} onSkipToApp={onSkipToApp} onNext={handleNext}
      onSelectOrg={handleSelectOrg} onNameChange={setName}
    />
  );
}
