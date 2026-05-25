// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import type { RepoWithStatus } from '../types';
import WizardShell from './WizardShell';

const CLI_COMMAND = 'npx @postlane/cli init';

interface Repo {
  id: string;
  name: string;
  path: string;
  active: boolean;
  added_at: string;
}

interface Props {
  workspaceId: string;
  onBack: () => void;
  onComplete: () => void;
  pollIntervalMs?: number;
}

interface DoneProps {
  repoName: string;
  onComplete: () => void;
  onAddAnother: () => void;
}

function RepoDone({ repoName, onComplete, onAddAnother }: DoneProps) {
  async function handleOpen() {
    try { await invoke('set_wizard_completed'); } catch { /* non-fatal */ }
    onComplete();
  }
  return (
    <WizardShell step={5} totalSteps={5} title="You're all set" subtitle="Your first repo is connected. Time to draft."
      onNext={handleOpen} nextLabel="Open dashboard">
      <div>
        <p className="mb-3">
          <span className="tag is-success is-light mr-2">&#10003;</span>
          <strong>{repoName}</strong> detected
        </p>
        <div className="notification is-info is-light is-size-7 py-2 px-3 mb-4">
          We've added a <code>project_id</code> to <code>.postlane/config.json</code> — commit this so your team can access this workspace.
        </div>
        <p className="is-size-7 has-text-grey mb-3">
          Use the <code>/draft-post</code> slash command in your repo to draft your first post.
        </p>
        <button className="button is-ghost is-small" onClick={onAddAnother}>Add another repo</button>
      </div>
    </WizardShell>
  );
}

interface PickerProps {
  onBack: () => void;
  connecting: boolean;
  connectError: string | null;
  cliExpanded: boolean;
  copied: boolean;
  onChooseFolder: () => void;
  onToggleCli: () => void;
  onCopy: () => void;
}

function RepoPickerStep({ onBack, connecting, connectError, cliExpanded, copied, onChooseFolder, onToggleCli, onCopy }: PickerProps) {
  return (
    <WizardShell step={5} totalSteps={5} title="Connect a repo"
      subtitle="Point Postlane at a git repository on your computer."
      onNext={() => {}} onBack={onBack} nextHidden>
      <div>
        <button className="button is-primary mb-3" onClick={onChooseFolder} disabled={connecting}>
          {connecting ? 'Connecting…' : 'Choose folder'}
        </button>
        {connectError && (
          <p className="help is-danger mb-3">{connectError}</p>
        )}
        <div>
          <button className="button is-ghost is-small" onClick={onToggleCli}>
            Set up manually with CLI
          </button>
          {cliExpanded && (
            <div className="field has-addons mt-2">
              <div className="control is-expanded">
                <code className="input is-small is-family-code"
                  style={{ display: 'flex', alignItems: 'center', background: '#f5f5f5', userSelect: 'all' }}>
                  {CLI_COMMAND}
                </code>
              </div>
              <div className="control">
                <button className="button is-small is-light" onClick={onCopy}>
                  {copied ? 'Copied!' : 'Copy'}
                </button>
              </div>
            </div>
          )}
          {cliExpanded && (
            <p className="is-size-7 has-text-grey mt-2">
              This window will update automatically when your repo is detected.
            </p>
          )}
        </div>
      </div>
    </WizardShell>
  );
}

interface RepoConnectState {
  detectedRepo: string | null;
  detecting: boolean;
  connecting: boolean;
  connectError: string | null;
  connectFolder: () => Promise<void>;
  reset: () => void;
}

function useRepoConnect(workspaceId: string, pollIntervalMs: number): RepoConnectState {
  const [detectedRepo, setDetectedRepo] = useState<string | null>(null);
  const [detecting, setDetecting] = useState(true);
  const [connecting, setConnecting] = useState(false);
  const [connectError, setConnectError] = useState<string | null>(null);

  useEffect(() => {
    if (!detecting) return;
    const interval = setInterval(async () => {
      try {
        const repos = await invoke<RepoWithStatus[]>('get_repos');
        // CLI stamped project_id before registering — already linked to this workspace
        const alreadyLinked = repos.find(r => r.project_id === workspaceId);
        if (alreadyLinked) {
          clearInterval(interval);
          setDetectedRepo(alreadyLinked.name);
          setDetecting(false);
          return;
        }
        // Fallback: repo registered without project_id (old CLI or no active session)
        const unlinked = repos.find(r => r.project_id === null);
        if (unlinked) {
          clearInterval(interval);
          setDetectedRepo(unlinked.name);
          try {
            await invoke('register_repo_with_project', {
              projectId: workspaceId,
              repoPath: unlinked.path,
              description: unlinked.name,
            });
          } catch { /* non-fatal */ }
          setDetecting(false);
        }
      } catch { /* ignore poll errors */ }
    }, pollIntervalMs);
    return () => clearInterval(interval);
  }, [detecting, workspaceId, pollIntervalMs]);

  async function connectFolder() {
    const result = await openDialog({ directory: true });
    if (typeof result !== 'string') return;
    setConnecting(true);
    setConnectError(null);
    try {
      const repo = await invoke<Repo>('connect_repo_from_desktop', { repoPath: result, projectId: workspaceId });
      setDetectedRepo(repo.name);
      setDetecting(false);
      setConnecting(false);
    } catch (err) {
      setConnectError(typeof err === 'string' ? err : 'Failed to connect repository');
      setConnecting(false);
    }
  }

  function reset() {
    setDetectedRepo(null);
    setDetecting(true);
    setConnecting(false);
    setConnectError(null);
  }

  return { detectedRepo, detecting, connecting, connectError, connectFolder, reset };
}

export default function ModalRepository({ workspaceId, onBack, onComplete, pollIntervalMs = 3000 }: Props) {
  const { detectedRepo, detecting, connecting, connectError, connectFolder, reset } = useRepoConnect(workspaceId, pollIntervalMs);
  const [cliExpanded, setCliExpanded] = useState(false);
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(CLI_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  if (!detecting && detectedRepo) {
    return <RepoDone repoName={detectedRepo} onComplete={onComplete} onAddAnother={reset} />;
  }
  return (
    <RepoPickerStep
      onBack={onBack}
      connecting={connecting}
      connectError={connectError}
      cliExpanded={cliExpanded}
      copied={copied}
      onChooseFolder={connectFolder}
      onToggleCli={() => setCliExpanded(e => !e)}
      onCopy={handleCopy}
    />
  );
}
