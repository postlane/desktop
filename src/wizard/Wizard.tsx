// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { Button } from '../components/catalyst/button';
import type { RepoWithStatus } from '../types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type Step = 1 | 2 | 3;
type Step1Branch = 'question' | 'yes' | 'no';

interface Props {
  onComplete: () => void;
}

// ---------------------------------------------------------------------------
// Step 1
// ---------------------------------------------------------------------------

interface Step1Props {
  onNext: (repo: RepoWithStatus) => void;
}

function Step1({ onNext }: Step1Props) {
  const [branch, setBranch] = useState<Step1Branch>('question');
  const [folderError, setFolderError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleBrowse() {
    setFolderError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;

    setLoading(true);
    try {
      const repo = await invoke<RepoWithStatus>('add_repo', { path: selected });
      onNext(repo);
    } catch (e) {
      setFolderError(
        "This folder hasn't been set up yet. Run `postlane init` inside it first.",
      );
    } finally {
      setLoading(false);
    }
  }

  if (branch === 'question') {
    return (
      <div className="flex flex-col gap-6">
        <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Add a repo</h2>
        <p className="text-zinc-600 dark:text-zinc-400">
          Have you already run <code className="font-mono text-sm">npx postlane init</code> in a repo?
        </p>
        <div className="flex gap-3">
          <Button onClick={() => setBranch('yes')}>Yes — browse for the folder</Button>
          <Button outline onClick={() => setBranch('no')}>No — show me how</Button>
        </div>
      </div>
    );
  }

  if (branch === 'no') {
    return (
      <div className="flex flex-col gap-6">
        <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Set up a repo first</h2>
        <p className="text-zinc-600 dark:text-zinc-400">
          Run this command inside the repo you want to use:
        </p>
        <pre className="rounded-lg bg-zinc-100 px-4 py-3 font-mono text-sm dark:bg-zinc-800">
          npx postlane init
        </pre>
        <div className="flex items-center gap-4">
          <Button onClick={() => setBranch('yes')}>Add repo</Button>
          <a
            href="https://postlane.dev/docs/getting-started"
            target="_blank"
            rel="noreferrer"
            className="text-sm text-blue-600 hover:underline dark:text-blue-400"
          >
            Open terminal guide →
          </a>
        </div>
        <Button plain onClick={() => setBranch('question')} className="self-start text-zinc-500">
          ← Back
        </Button>
      </div>
    );
  }

  // branch === 'yes'
  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Add a repo</h2>
      <p className="text-zinc-600 dark:text-zinc-400">
        Select the folder where you ran <code className="font-mono text-sm">npx postlane init</code>.
      </p>
      <div className="flex gap-3">
        <Button outline onClick={() => setBranch('question')}>← Back</Button>
        <Button onClick={handleBrowse} disabled={loading}>
          {loading ? 'Adding…' : 'Browse for the folder'}
        </Button>
      </div>
      {folderError && (
        <p className="text-sm text-red-600 dark:text-red-400">{folderError}</p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 2
// ---------------------------------------------------------------------------

interface Step2Props {
  onNext: () => void;
}

type CredentialState =
  | { status: 'loading' }
  | { status: 'found'; preview: string }
  | { status: 'not_found' };

const PROVIDERS = ['Zernio', 'Buffer', 'Ayrshare', 'Other'] as const;
type Provider = (typeof PROVIDERS)[number];

function Step2({ onNext }: Step2Props) {
  const [credState, setCredState] = useState<CredentialState>({ status: 'loading' });
  const [provider, setProvider] = useState<Provider>('Zernio');
  const [apiKey, setApiKey] = useState('');
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  // Check keyring on mount
  useState(() => {
    invoke<string>('get_scheduler_credential', { provider: 'zernio' })
      .then((key) => setCredState({ status: 'found', preview: key }))
      .catch(() => setCredState({ status: 'not_found' }));
  });

  async function handleTest() {
    setTestResult(null);
    setTestError(null);
    try {
      await invoke('test_scheduler', { provider: provider.toLowerCase() });
      setTestResult('ok');
    } catch (e) {
      setTestResult('error');
      setTestError(e instanceof Error ? e.message : 'Connection failed');
    }
  }

  if (credState.status === 'loading') {
    return <p className="text-sm text-zinc-500">Checking credentials…</p>;
  }

  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">
        Connect a scheduler
      </h2>

      {credState.status === 'found' ? (
        <>
          <p className="text-sm text-green-700 dark:text-green-400">
            ✓ Connected{' '}
            <span className="font-mono text-xs">({credState.preview})</span>
          </p>
          <div className="flex gap-3">
            <button
              onClick={onNext}
              className="rounded-lg bg-zinc-900 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              Continue →
            </button>
            <button
              onClick={() => setCredState({ status: 'not_found' })}
              className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              Change scheduler
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="flex flex-col gap-4">
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value as Provider)}
              className="rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              {PROVIDERS.map((p) => (
                <option key={p} value={p}>{p}</option>
              ))}
            </select>
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="API key"
              className="rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            />
            <div className="flex items-center gap-3">
              <button
                onClick={handleTest}
                className="rounded-lg border border-zinc-300 px-3 py-1.5 text-sm text-zinc-700 hover:bg-zinc-50 dark:border-zinc-600 dark:text-zinc-300 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              >
                Test connection
              </button>
              {testResult === 'ok' && <span className="text-sm text-green-600">✓</span>}
              {testResult === 'error' && (
                <span className="text-sm text-red-600">{testError}</span>
              )}
            </div>
          </div>
          <div className="flex items-center gap-4">
            <button
              onClick={onNext}
              className="rounded-lg bg-zinc-900 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              Continue →
            </button>
            <button
              onClick={onNext}
              className="text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              Skip for now
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 3
// ---------------------------------------------------------------------------

interface Step3Props {
  repoName: string;
  onComplete: () => void;
}

function Step3({ repoName, onComplete }: Step3Props) {
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'fallback'>('idle');

  async function handleCopy() {
    try {
      await writeText('/draft-post');
      setCopyState('copied');
      setTimeout(() => setCopyState('idle'), 2000);
    } catch {
      setCopyState('fallback');
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">
        You're ready
      </h2>
      <p className="text-zinc-600 dark:text-zinc-400">
        Open your IDE in <strong>{repoName}</strong> and run:
      </p>
      <div className="flex items-center gap-3 rounded-lg bg-zinc-100 px-4 py-3 dark:bg-zinc-800">
        <code className="flex-1 font-mono text-sm text-zinc-900 dark:text-zinc-100">
          /draft-post
        </code>
        <button
          onClick={handleCopy}
          aria-label="Copy /draft-post command"
          className="rounded px-2 py-1 text-xs text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          {copyState === 'copied' ? '✓ Copied' : 'Copy'}
        </button>
      </div>
      {copyState === 'fallback' && (
        <p className="text-xs text-zinc-500">Press Ctrl+C to copy</p>
      )}
      <p className="text-sm text-zinc-500">
        Your draft will appear in <strong>{repoName}</strong> → Drafts.
      </p>
      <div className="flex items-center gap-4">
        <button
          onClick={onComplete}
          className="rounded-lg bg-zinc-900 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          Done
        </button>
        <a
          href="https://postlane.dev/docs"
          target="_blank"
          rel="noreferrer"
          className="text-sm text-blue-600 hover:underline dark:text-blue-400"
        >
          Open docs →
        </a>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Wizard shell
// ---------------------------------------------------------------------------

export default function Wizard({ onComplete }: Props) {
  const [step, setStep] = useState<Step>(1);
  const [addedRepo, setAddedRepo] = useState<RepoWithStatus | null>(null);

  function handleStep1Next(repo: RepoWithStatus) {
    setAddedRepo(repo);
    setStep(2);
  }

  return (
    <div className="flex h-screen items-center justify-center bg-white dark:bg-zinc-900">
      <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-8 shadow-sm dark:border-zinc-700 dark:bg-zinc-900">
        {/* Step indicator */}
        <div className="mb-8 flex items-center gap-2">
          {([1, 2, 3] as Step[]).map((n) => (
            <div
              key={n}
              className={`h-1.5 flex-1 rounded-full ${
                n <= step ? 'bg-zinc-900 dark:bg-zinc-100' : 'bg-zinc-200 dark:bg-zinc-700'
              }`}
            />
          ))}
        </div>

        {step === 1 && <Step1 onNext={handleStep1Next} />}
        {step === 2 && <Step2 onNext={() => setStep(3)} />}
        {step === 3 && (
          <Step3
            repoName={addedRepo?.name ?? 'your repo'}
            onComplete={onComplete}
          />
        )}
      </div>
    </div>
  );
}
