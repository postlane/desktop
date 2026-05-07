// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import WizardShell from './WizardShell';

const CLI_COMMAND = 'npx @postlane/cli init';

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

interface DetectingProps {
  onBack: () => void;
  copied: boolean;
  onCopy: () => void;
}

function RepoDetecting({ onBack, copied, onCopy }: DetectingProps) {
  return (
    <WizardShell step={5} totalSteps={5} title="Connect a repo"
      subtitle="Run this command in your repo's root directory."
      onNext={() => {}} onBack={onBack} nextHidden>
      <div>
        <div className="field has-addons mb-4">
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
        <p className="is-size-7 has-text-grey">
          It takes about 30 seconds. This window will update automatically when your repo is detected.
        </p>
      </div>
    </WizardShell>
  );
}

export default function ModalRepository({ workspaceId, onBack, onComplete, pollIntervalMs = 3000 }: Props) {
  const [detectedRepo, setDetectedRepo] = useState<string | null>(null);
  const [detecting, setDetecting] = useState(true);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!detecting) return;
    const interval = setInterval(async () => {
      try {
        const repos = await invoke<string[]>('get_repos');
        if (repos.length > 0) {
          clearInterval(interval);
          const name = repos[0];
          setDetectedRepo(name);
          try { await invoke('register_repo_with_project', { repoName: name, workspaceId }); } catch { /* non-fatal */ }
          setDetecting(false);
        }
      } catch { /* ignore poll errors */ }
    }, pollIntervalMs);
    return () => clearInterval(interval);
  }, [detecting, workspaceId, pollIntervalMs]);

  async function handleCopy() {
    await navigator.clipboard.writeText(CLI_COMMAND);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  if (!detecting && detectedRepo) {
    return <RepoDone repoName={detectedRepo} onComplete={onComplete} onAddAnother={() => setDetecting(true)} />;
  }
  return <RepoDetecting onBack={onBack} copied={copied} onCopy={handleCopy} />;
}
