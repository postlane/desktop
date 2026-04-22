// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { Button } from '../components/catalyst/button';
import type { RepoWithStatus } from '../types';

type Step = 1 | 2 | 3;
type Step1Branch = 'question' | 'yes' | 'no';

interface Props {
  onComplete: () => void;
}

const PROVIDERS = ['Zernio', 'Buffer', 'Ayrshare', 'Other'] as const;
type Provider = (typeof PROVIDERS)[number];

type CredentialState = { status: 'loading' } | { status: 'found'; preview: string } | { status: 'not_found' };

const BTN_PRIMARY = 'rounded-lg bg-zinc-900 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-700 dark:bg-zinc-100 dark:text-zinc-900 dark:hover:bg-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500';
const BTN_GHOST = 'text-sm text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500';
const INPUT_CLASS = 'rounded-lg border border-zinc-300 bg-white px-3 py-2 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500';

// ---------------------------------------------------------------------------
// Step 1 sub-components
// ---------------------------------------------------------------------------

function Step1Question({ onYes, onNo }: { onYes: () => void; onNo: () => void }) {
  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Add a repo</h2>
      <p className="text-zinc-600 dark:text-zinc-400">Have you already run <code className="font-mono text-sm">npx postlane init</code> in a repo?</p>
      <div className="flex gap-3">
        <Button onClick={onYes}>Yes — browse for the folder</Button>
        <Button outline onClick={onNo}>No — show me how</Button>
      </div>
    </div>
  );
}

function Step1No({ onBack }: { onBack: () => void }) {
  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Set up a repo first</h2>
      <p className="text-zinc-600 dark:text-zinc-400">Run this command inside the repo you want to use:</p>
      <pre className="rounded-lg bg-zinc-100 px-4 py-3 font-mono text-sm dark:bg-zinc-800">npx postlane init</pre>
      <div className="flex items-center gap-4">
        <Button onClick={onBack}>Add repo</Button>
        <a href="https://postlane.dev/docs/getting-started" target="_blank" rel="noreferrer" className="text-sm text-blue-600 hover:underline dark:text-blue-400">Open terminal guide →</a>
      </div>
      <Button plain onClick={onBack} className="self-start text-zinc-500">← Back</Button>
    </div>
  );
}

function Step1Yes({ onBack, onBrowse, loading, folderError }: { onBack: () => void; onBrowse: () => void; loading: boolean; folderError: string | null }) {
  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Add a repo</h2>
      <p className="text-zinc-600 dark:text-zinc-400">Select the folder where you ran <code className="font-mono text-sm">npx postlane init</code>.</p>
      <div className="flex gap-3">
        <Button outline onClick={onBack}>← Back</Button>
        <Button onClick={onBrowse} disabled={loading}>{loading ? 'Adding…' : 'Browse for the folder'}</Button>
      </div>
      {folderError && <p className="text-sm text-red-600 dark:text-red-400">{folderError}</p>}
    </div>
  );
}

function Step1({ onNext }: { onNext: (repo: RepoWithStatus) => void }) {
  const [branch, setBranch] = useState<Step1Branch>('question');
  const [folderError, setFolderError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleBrowse() {
    setFolderError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;
    setLoading(true);
    try { const repo = await invoke<RepoWithStatus>('add_repo', { path: selected }); onNext(repo); }
    catch { setFolderError("This folder hasn't been set up yet. Run `postlane init` inside it first."); }
    finally { setLoading(false); }
  }

  if (branch === 'question') return <Step1Question onYes={() => setBranch('yes')} onNo={() => setBranch('no')} />;
  if (branch === 'no') return <Step1No onBack={() => setBranch('question')} />;
  return <Step1Yes onBack={() => setBranch('question')} onBrowse={handleBrowse} loading={loading} folderError={folderError} />;
}

// ---------------------------------------------------------------------------
// Step 2 sub-components
// ---------------------------------------------------------------------------

function Step2Connected({ onNext, onChangeScheduler, preview }: { onNext: () => void; onChangeScheduler: () => void; preview: string }) {
  return (
    <>
      <p className="text-sm text-green-700 dark:text-green-400">✓ Connected <span className="font-mono text-xs">({preview})</span></p>
      <div className="flex gap-3">
        <button onClick={onNext} className={BTN_PRIMARY}>Continue →</button>
        <button onClick={onChangeScheduler} className={BTN_GHOST}>Change scheduler</button>
      </div>
    </>
  );
}

interface Step2SetupProps {
  provider: Provider;
  apiKey: string;
  testResult: 'ok' | 'error' | null;
  testError: string | null;
  onProviderChange: (_p: Provider) => void;
  onApiKeyChange: (_k: string) => void;
  onTest: () => void;
  onNext: () => void;
}

function Step2Setup({ provider, apiKey, testResult, testError, onProviderChange, onApiKeyChange, onTest, onNext }: Step2SetupProps) {
  return (
    <>
      <div className="flex flex-col gap-4">
        <select value={provider} onChange={(e) => onProviderChange(e.target.value as Provider)} className={INPUT_CLASS}>
          {PROVIDERS.map((p) => <option key={p} value={p}>{p}</option>)}
        </select>
        <input type="password" value={apiKey} onChange={(e) => onApiKeyChange(e.target.value)} placeholder="API key" className={INPUT_CLASS} />
        <div className="flex items-center gap-3">
          <button onClick={onTest} className="rounded-lg border border-zinc-300 px-3 py-1.5 text-sm text-zinc-700 hover:bg-zinc-50 dark:border-zinc-600 dark:text-zinc-300 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500">Test connection</button>
          {testResult === 'ok' && <span className="text-sm text-green-600">✓</span>}
          {testResult === 'error' && <span className="text-sm text-red-600">{testError}</span>}
        </div>
      </div>
      <div className="flex items-center gap-4">
        <button onClick={onNext} className={BTN_PRIMARY}>Continue →</button>
        <button onClick={onNext} className={BTN_GHOST}>Skip for now</button>
      </div>
    </>
  );
}

function Step2({ onNext }: { onNext: () => void }) {
  const [credState, setCredState] = useState<CredentialState>({ status: 'loading' });
  const [provider, setProvider] = useState<Provider>('Zernio');
  const [apiKey, setApiKey] = useState('');
  const [testResult, setTestResult] = useState<'ok' | 'error' | null>(null);
  const [testError, setTestError] = useState<string | null>(null);

  useState(() => {
    invoke<string>('get_scheduler_credential', { provider: 'zernio' })
      .then((key) => setCredState({ status: 'found', preview: key }))
      .catch(() => setCredState({ status: 'not_found' }));
  });

  async function handleTest() {
    setTestResult(null); setTestError(null);
    try { await invoke('test_scheduler', { provider: provider.toLowerCase() }); setTestResult('ok'); }
    catch (e) { setTestResult('error'); setTestError(e instanceof Error ? e.message : 'Connection failed'); }
  }

  if (credState.status === 'loading') return <p className="text-sm text-zinc-500">Checking credentials…</p>;
  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">Connect a scheduler</h2>
      {credState.status === 'found'
        ? <Step2Connected onNext={onNext} onChangeScheduler={() => setCredState({ status: 'not_found' })} preview={credState.preview} />
        : <Step2Setup provider={provider} apiKey={apiKey} testResult={testResult} testError={testError} onProviderChange={(p) => setProvider(p)} onApiKeyChange={setApiKey} onTest={handleTest} onNext={onNext} />}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 3
// ---------------------------------------------------------------------------

function Step3({ repoName, onComplete }: { repoName: string; onComplete: () => void }) {
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'fallback'>('idle');

  async function handleCopy() {
    try { await writeText('/draft-post'); setCopyState('copied'); setTimeout(() => setCopyState('idle'), 2000); }
    catch { setCopyState('fallback'); }
  }

  return (
    <div className="flex flex-col gap-6">
      <h2 className="text-xl font-semibold text-zinc-900 dark:text-zinc-100">You're ready</h2>
      <p className="text-zinc-600 dark:text-zinc-400">Open your IDE in <strong>{repoName}</strong> and run:</p>
      <div className="flex items-center gap-3 rounded-lg bg-zinc-100 px-4 py-3 dark:bg-zinc-800">
        <code className="flex-1 font-mono text-sm text-zinc-900 dark:text-zinc-100">/draft-post</code>
        <button onClick={handleCopy} aria-label="Copy /draft-post command" className="rounded px-2 py-1 text-xs text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500">
          {copyState === 'copied' ? '✓ Copied' : 'Copy'}
        </button>
      </div>
      {copyState === 'fallback' && <p className="text-xs text-zinc-500">Press Ctrl+C to copy</p>}
      <p className="text-sm text-zinc-500">Your draft will appear in <strong>{repoName}</strong> → Drafts.</p>
      <div className="flex items-center gap-4">
        <button onClick={onComplete} className={BTN_PRIMARY}>Done</button>
        <a href="https://postlane.dev/docs" target="_blank" rel="noreferrer" className="text-sm text-blue-600 hover:underline dark:text-blue-400">Open docs →</a>
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

  return (
    <div className="flex h-screen items-center justify-center bg-white dark:bg-zinc-900">
      <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-8 shadow-sm dark:border-zinc-700 dark:bg-zinc-900">
        <div className="mb-8 flex items-center gap-2">
          {([1, 2, 3] as Step[]).map((n) => (
            <div key={n} className={`h-1.5 flex-1 rounded-full ${n <= step ? 'bg-zinc-900 dark:bg-zinc-100' : 'bg-zinc-200 dark:bg-zinc-700'}`} />
          ))}
        </div>
        {step === 1 && <Step1 onNext={(repo) => { setAddedRepo(repo); setStep(2); }} />}
        {step === 2 && <Step2 onNext={() => setStep(3)} />}
        {step === 3 && <Step3 repoName={addedRepo?.name ?? 'your repo'} onComplete={onComplete} />}
      </div>
    </div>
  );
}
