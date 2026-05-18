// SPDX-License-Identifier: BUSL-1.1

import { useCallback, useEffect, useRef, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';

const GITHUB_APP_INSTALL_URL = 'https://github.com/apps/postlane/installations/new';
const CLI_COMMAND = 'npx @postlane/cli init';
const POLL_INTERVAL_MS = 3000;
export const MAX_POLL_ATTEMPTS = 120; // 6 minutes
export const POLL_SLOW_THRESHOLD = 10; // 30 seconds — show "still checking" notice

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
  setRepoConnected: (_v: boolean) => void;
}

interface GitHubAppSectionProps {
  appInstalled: boolean;
  error: string | null;
  onInstall: () => void;
  pollSlowNotice: boolean;
  pollTimedOut: boolean;
}

function GitHubAppSection({ appInstalled, error, onInstall, pollSlowNotice, pollTimedOut }: GitHubAppSectionProps) {
  return (
    <div className="box mb-3">
      <p className="has-text-weight-semibold mb-1">GitHub App</p>
      <p className="is-size-7 has-text-grey mb-3">
        Monitors selected repos via GitHub webhooks. Works for the whole team, even when this app is closed.
      </p>
      {appInstalled && (
        <p className="is-size-7 mb-2">
          <span className="tag is-success is-light mr-2">&#10003;</span>
          GitHub App connected
        </p>
      )}
      <button className="button is-primary is-small" onClick={onInstall}>
        Install GitHub App
      </button>
      {error && <p role="alert" className="is-size-7 has-text-danger mt-2">{error}</p>}
      {pollSlowNotice && !pollTimedOut && (
        <p className="is-size-7 has-text-grey mt-2">
          Still waiting for GitHub — this can take a minute after installing.
        </p>
      )}
      {pollTimedOut && (
        <p role="alert" className="is-size-7 has-text-danger mt-2">
          GitHub App install not detected after 6 minutes. If you completed the install, use
          &ldquo;Skip&rdquo; below to continue and connect repos via the folder picker or CLI instead.
        </p>
      )}
    </div>
  );
}

interface FolderSectionProps {
  workspaceId: string;
  workspaceName: string;
  onConnected: () => void;
  onAlreadyConnected: () => void;
}

function FolderPickerSection({ workspaceId, workspaceName, onConnected, onAlreadyConnected }: FolderSectionProps) {
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
      if (typeof err === 'string' && err.startsWith('RepoAlreadyRegistered:')) onAlreadyConnected();
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
      {connectedName && (
        <p className="is-size-7 mb-2">
          <span className="tag is-success is-light mr-2">&#10003;</span>
          <strong>{connectedName}</strong> connected.
        </p>
      )}
      <button className="button is-light is-small" onClick={handleChoose} disabled={connecting}>
        {connecting ? 'Connecting…' : connectedName ? 'Add another folder' : 'Choose folder'}
      </button>
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

interface InstallHookResult {
  appInstalled: boolean;
  appInstallError: string | null;
  pollSlowNotice: boolean;
  pollTimedOut: boolean;
  handleInstall: () => Promise<void>;
}

function useGitHubAppEvents(
  isGitHub: boolean,
  advance: () => void,
  onError: (msg: string) => void,
) {
  useEffect(() => {
    const unlisten = Promise.all([
      listen<{ installation_id: number }>('github:app-installed', () => {
        if (isGitHub) advance();
      }),
      listen<{ message: string }>('github:install-error', (e) => {
        if (isGitHub) onError(e.payload.message);
      }),
    ]);
    return () => { void unlisten.then(([u1, u2]) => { u1(); u2(); }); };
  }, [isGitHub, advance, onError]);
}

function useGitHubAppInstall(isGitHub: boolean, workspaceId: string, onNext: () => void, onRepoConnected: () => void): InstallHookResult {
  const [appInstalled, setAppInstalled] = useState(false);
  const [appInstallError, setAppInstallError] = useState<string | null>(null);
  const [pollSlowNotice, setPollSlowNotice] = useState(false);
  const [pollTimedOut, setPollTimedOut] = useState(false);
  const advancedRef = useRef(false);
  const pollingActiveRef = useRef(false);
  const cancelPollRef = useRef(false);
  const pollAttemptRef = useRef(0);

  useEffect(() => {
    return () => { cancelPollRef.current = true; };
  }, []);

  const advance = useCallback(() => {
    if (advancedRef.current) return;
    advancedRef.current = true;
    onRepoConnected();
    onNext();
  }, [onNext, onRepoConnected]);

  const advanceFnRef = useRef(advance);
  advanceFnRef.current = advance;

  // Check on mount — if app was installed before this session, show Connected badge.
  // Do NOT auto-advance: let the user explicitly click Next.
  useEffect(() => {
    if (!isGitHub) return;
    let cancelled = false;
    invoke<boolean>('check_github_app_installed', { projectId: workspaceId })
      .then((installed) => { if (!cancelled && installed) setAppInstalled(true); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [isGitHub, workspaceId]);

  useGitHubAppEvents(isGitHub, advance, setAppInstallError);

  async function handleInstall() {
    openUrl(GITHUB_APP_INSTALL_URL).catch(console.error);
    if (!isGitHub || pollingActiveRef.current) return;
    pollingActiveRef.current = true;
    pollAttemptRef.current = 0;
    setPollSlowNotice(false);
    setPollTimedOut(false);

    const poll = async () => {
      if (cancelPollRef.current) return;
      pollAttemptRef.current += 1;
      if (pollAttemptRef.current > MAX_POLL_ATTEMPTS) {
        pollingActiveRef.current = false;
        if (!cancelPollRef.current) setPollTimedOut(true);
        return;
      }
      if (pollAttemptRef.current === POLL_SLOW_THRESHOLD && !cancelPollRef.current) {
        setPollSlowNotice(true);
      }
      try {
        const installed = await invoke<boolean>('check_github_app_installed', { projectId: workspaceId });
        if (installed) { advance(); return; }
      } catch {
        // ignore transient errors and keep polling
      }
      if (!cancelPollRef.current) setTimeout(poll, POLL_INTERVAL_MS);
    };
    await poll();
  }

  return { appInstalled, appInstallError, pollSlowNotice, pollTimedOut, handleInstall };
}

export default function ModalGitHubApp({ provider, workspaceId, workspaceName, onNext, onBack, setRepoConnected }: Props) {
  const [folderConnected, setFolderConnected] = useState(false);
  const [alreadyConnected, setAlreadyConnected] = useState(false);
  const isGitHub = provider === 'github';
  const { appInstalled, appInstallError, pollSlowNotice, pollTimedOut, handleInstall } = useGitHubAppInstall(isGitHub, workspaceId, onNext, () => setRepoConnected(true));

  // When the app was already installed before this session, advance() is not
  // called (no auto-advance), so setRepoConnected must be called here instead.
  function handleNext() {
    if (appInstalled) setRepoConnected(true);
    onNext();
  }

  return (
    <WizardShell
      step={5}
      totalSteps={7}
      title="Connect your repos"
      subtitle="Choose how Postlane monitors your projects. All methods are read-only."
      onNext={handleNext}
      onBack={onBack}
      nextHidden={!folderConnected && !alreadyConnected && !appInstalled}
      onSkip={!folderConnected && !appInstalled ? onNext : undefined}
      skipLabel="I'll connect repos later"
    >
      {isGitHub && <GitHubAppSection appInstalled={appInstalled} error={appInstallError} onInstall={handleInstall} pollSlowNotice={pollSlowNotice} pollTimedOut={pollTimedOut} />}
      <FolderPickerSection workspaceId={workspaceId} workspaceName={workspaceName}
        onConnected={() => { setFolderConnected(true); setRepoConnected(true); }}
        onAlreadyConnected={() => { setAlreadyConnected(true); setRepoConnected(true); }}
      />
      <CliSection />
    </WizardShell>
  );
}
