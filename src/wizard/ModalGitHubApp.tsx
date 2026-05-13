// SPDX-License-Identifier: BUSL-1.1

import { useEffect, useRef, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';

const GITHUB_APP_INSTALL_URL = 'https://github.com/apps/postlane/installations/new';
const CLI_COMMAND = 'npx @postlane/cli init';

function repoConnectError(err: unknown, workspaceName?: string): string {
  const raw = typeof err === 'string' ? err : '';
  if (raw.startsWith('NotAGitRepo:')) return 'Not a Git repository. Please select a folder that contains a .git directory.';
  if (raw.startsWith('RepoAlreadyRegistered:')) {
    const target = workspaceName ? `the ${workspaceName} workspace` : 'a workspace';
    return `This repository is already connected to ${target}.`;
  }
  if (raw.startsWith('PathNotAuthorised:')) return 'This folder is outside your home directory and cannot be connected.';
  return 'Failed to connect repository';
}

interface Props {
  provider: string;
  workspaceId: string;
  workspaceName: string;
  onNext: () => void;
  onBack: () => void;
}

function GitHubAppSection({ error }: { error: string | null }) {
  return (
    <div className="box mb-3">
      <p className="has-text-weight-semibold mb-1">GitHub App</p>
      <p className="is-size-7 has-text-grey mb-3">
        Watches your whole org automatically. No per-repo setup.
      </p>
      <button className="button is-primary is-small"
        onClick={() => openUrl(GITHUB_APP_INSTALL_URL).catch(console.error)}>
        Install GitHub App
      </button>
      {error && <p role="alert" className="is-size-7 has-text-danger mt-2">{error}</p>}
    </div>
  );
}

interface FolderSectionProps {
  workspaceId: string;
  workspaceName: string;
  onConnected: () => void;
}

function FolderPickerSection({ workspaceId, workspaceName, onConnected }: FolderSectionProps) {
  const [connecting, setConnecting] = useState(false);
  const [connectedName, setConnectedName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const pickerOpenRef = useRef(false);

  async function handleChoose() {
    if (pickerOpenRef.current || connecting) return;
    pickerOpenRef.current = true;
    setConnecting(true);
    setError(null);
    const result = await openDialog({ directory: true });
    if (typeof result !== 'string') {
      pickerOpenRef.current = false;
      setConnecting(false);
      return;
    }
    try {
      const repo = await invoke<{ name: string }>('connect_repo_from_desktop', { repoPath: result, projectId: workspaceId });
      setConnectedName(repo.name);
      onConnected();
    } catch (err) {
      setError(repoConnectError(err, workspaceName));
    } finally {
      pickerOpenRef.current = false;
      setConnecting(false);
    }
  }

  return (
    <div className="box mb-3">
      <p className="has-text-weight-semibold mb-1">Desktop folder</p>
      <p className="is-size-7 has-text-grey mb-3">
        Connect individual repos or folders from your machine.
      </p>
      {connectedName ? (
        <p className="is-size-7">
          <span className="tag is-success is-light mr-2">&#10003;</span>
          <strong>{connectedName}</strong> connected. You can add more or click Next.
        </p>
      ) : (
        <button className="button is-light is-small" onClick={handleChoose} disabled={connecting}>
          {connecting ? 'Connecting…' : 'Choose folder'}
        </button>
      )}
      {error && <p role="alert" className="is-size-7 has-text-danger mt-2">{error}</p>}
    </div>
  );
}

function CliSection() {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(CLI_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div className="box mb-3">
      <p className="has-text-weight-semibold mb-1">CLI</p>
      <p className="is-size-7 has-text-grey mb-2">
        Run a command inside a repo from your terminal.
      </p>
      <button className="button is-ghost is-small p-0 has-text-link"
        onClick={() => setExpanded(e => !e)}>
        {expanded ? 'Hide command' : 'Show command'}
      </button>
      {expanded && (
        <>
          <div className="field has-addons mt-2">
            <div className="control is-expanded">
              <code className="input is-small is-family-code"
                style={{ display: 'flex', alignItems: 'center', background: '#f5f5f5', userSelect: 'all' }}>
                {CLI_COMMAND}
              </code>
            </div>
            <div className="control">
              <button className="button is-small is-light" onClick={handleCopy}>
                {copied ? 'Copied!' : 'Copy'}
              </button>
            </div>
          </div>
          <p className="is-size-7 has-text-grey mt-2">
            This window updates automatically when your repo is detected.
          </p>
        </>
      )}
    </div>
  );
}

export default function ModalGitHubApp({ provider, workspaceId, workspaceName, onNext, onBack }: Props) {
  const [folderConnected, setFolderConnected] = useState(false);
  const [appInstallError, setAppInstallError] = useState<string | null>(null);
  const isGitHub = provider === 'github';

  useEffect(() => {
    const unlisten = Promise.all([
      listen<{ installation_id: number }>('github:app-installed', () => {
        if (isGitHub) onNext();
      }),
      listen<{ message: string }>('github:install-error', (e) => {
        if (isGitHub) setAppInstallError(e.payload.message);
      }),
    ]);
    return () => { void unlisten.then(([u1, u2]) => { u1(); u2(); }); };
  }, [isGitHub, onNext]);

  return (
    <WizardShell
      step={5}
      totalSteps={5}
      title="Connect your repos"
      subtitle="Choose how Postlane monitors your projects. All methods are read-only."
      onNext={onNext}
      onBack={onBack}
      nextHidden={!folderConnected}
      onSkip={!folderConnected ? onNext : undefined}
    >
      {isGitHub && <GitHubAppSection error={appInstallError} />}
      <FolderPickerSection workspaceId={workspaceId} workspaceName={workspaceName} onConnected={() => setFolderConnected(true)} />
      <CliSection />
    </WizardShell>
  );
}
