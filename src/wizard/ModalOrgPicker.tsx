// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';
import { GitHubLogo, GitLabLogo } from '../assets/logos';

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
  onNext: (workspaceId: string, workspaceName: string) => void;
  onBack: () => void;
  onPricingGate: () => void;
  onSkipToApp?: () => void;
  provider?: string;
}

interface OrgRowProps {
  org: OrgSummary;
  selected: boolean;
  provider: string;
  onSelect: () => void;
}

interface OrgListViewProps {
  orgs: OrgSummary[];
  selectedOrg: OrgSummary | null;
  name: string;
  createError: string | null;
  creating: boolean;
  provider: string;
  onBack: () => void;
  onSkipToApp?: () => void;
  onNext: () => void;
  onSelectOrg: (org: OrgSummary) => void;
  onNameChange: (name: string) => void;
}

function OrgRow({ org, selected, provider, onSelect }: OrgRowProps) {
  const ProviderLogo = provider === 'gitlab' ? GitLabLogo : GitHubLogo;
  const providerLabel = provider === 'gitlab' ? 'GitLab' : 'GitHub';
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
      <ProviderLogo size={14} ariaLabel={providerLabel} />
      {org.has_project && <span className="tag is-light is-small ml-1">Existing</span>}
    </button>
  );
}

function ScopeError({ provider, onBack }: { provider: string; onBack: () => void }) {
  async function handleReauth() {
    try {
      const port = await invoke<number>('get_local_server_port');
      openUrl(`https://postlane.dev/login?desktop=1&port=${port}&provider=${provider}`).catch(console.error);
    } catch {
      openUrl(`https://postlane.dev/login?desktop=1&provider=${provider}`).catch(console.error);
    }
  }
  return (
    <WizardShell step={3} totalSteps={3}
      title="Permission needed"
      subtitle="Postlane needs permission to read your organisation list."
      onNext={() => {}} onBack={onBack} nextHidden
    >
      <p className="mb-4 is-size-7">Postlane needs permission to see your GitHub organisations. Sign in again and approve the request when prompted.</p>
      <button className="button is-primary is-small" onClick={handleReauth}>Sign in again</button>
    </WizardShell>
  );
}

interface LoadErrorProps { message: string; onBack: () => void; onRetry: () => void; }

function LoadError({ message, onBack, onRetry }: LoadErrorProps) {
  return (
    <WizardShell step={3} totalSteps={3}
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
    <WizardShell step={3} totalSteps={3}
      title="Choose your account or org"
      subtitle="Select the account or organisation this workspace is for."
      onNext={onNext} onBack={onBack} nextDisabled nextHidden
    >
      <p className="has-text-grey is-size-7">Loading organisations…</p>
    </WizardShell>
  );
}

function orgAccessUrl(provider: string): string {
  return provider === 'gitlab'
    ? 'https://gitlab.com/-/profile/applications'
    : 'https://github.com/settings/connections/applications';
}

function OrgListView({ orgs, selectedOrg, name, createError, creating, provider, onBack, onSkipToApp, onNext, onSelectOrg, onNameChange }: OrgListViewProps) {
  const providerLabel = provider === 'gitlab' ? 'GitLab' : 'GitHub';
  const entityLabel = provider === 'gitlab' ? 'groups' : 'organisations';
  return (
    <WizardShell step={3} totalSteps={3}
      title="Choose your account or org"
      subtitle="Select the account or organisation this workspace is for."
      onNext={onNext} onBack={onBack}
      nextDisabled={selectedOrg === null || (!selectedOrg.has_project && name.trim().length === 0) || creating}
      onSkip={onSkipToApp}
    >
      <div role="listbox" aria-label="Organisations" className="mb-4">
        {orgs.map((org) => (
          <OrgRow key={org.login} org={org} provider={provider} selected={selectedOrg?.login === org.login} onSelect={() => onSelectOrg(org)} />
        ))}
      </div>
      <p className="is-size-7 has-text-grey mb-3">
        {`Don't see an org? You may need to grant Postlane access to more ${entityLabel} on ${providerLabel} first, then sign in again. `}
        <button
          type="button"
          className="button is-ghost p-0 is-size-7 has-text-link"
          style={{ height: 'auto', verticalAlign: 'baseline' }}
          onClick={() => openUrl(orgAccessUrl(provider)).catch(console.error)}
        >
          {`Manage ${providerLabel} app permissions →`}
        </button>
      </p>
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
      .then((orgs) => { setOrgs(orgs); })
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        if (msg.includes('scope_not_granted')) { setScopeError(true); } else { setLoadError(msg); }
      });
  }, [provider, retryCount]);

  useEffect(() => {
    if (!scopeError) return;
    let unlisten: (() => void) | undefined;
    let mounted = true;
    listen('license:activated', () => {
      if (mounted) setRetryCount((c) => c + 1);
    })
      .then((fn) => { if (mounted) { unlisten = fn; } else { fn(); } })
      .catch(console.error);
    return () => { mounted = false; unlisten?.(); };
  }, [scopeError]);

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

    // Existing workspace — backfill org login if missing, then route
    if (selectedOrg.has_project) {
      if (selectedOrg.project_id) {
        if (selectedOrg.login) {
          invoke('backfill_project_org_login', {
            projectId: selectedOrg.project_id,
            orgLogin: selectedOrg.login,
          }).catch(() => {});
        }
        onNext(selectedOrg.project_id, selectedOrg.display_name);
      }
      return;
    }

    if (name.trim().length === 0) return;
    setCreateError(null);
    setCreating(true);
    try {
      const params = {
        name: name.trim(),
        workspaceType: selectedOrg.is_personal ? 'personal' : 'organization',
        providerOrgLogin: selectedOrg.login,
      };
      const result = await invoke<CreateProjectResult>('create_project', params);
      onNext(result.project_id, result.name);
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
      provider={provider}
      onBack={onBack} onSkipToApp={onSkipToApp} onNext={handleNext}
      onSelectOrg={handleSelectOrg} onNameChange={setName}
    />
  );
}
