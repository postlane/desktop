// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone } from '../TimezoneContext';
import type { AppStateFile } from '../types';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Button } from '../components/catalyst/button';
import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogDescription,
  DialogTitle,
} from '../components/catalyst/dialog';
import type { RepoWithStatus, SchedulerProfile } from '../types';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type Tab = 'repos' | 'scheduler' | 'app';

const PROVIDERS = ['zernio', 'buffer', 'ayrshare'] as const;
type Provider = (typeof PROVIDERS)[number];

interface CredentialState {
  preview: string | null; // null = not configured
  testing: boolean;
  testResult: 'ok' | 'error' | null;
  testError: string | null;
  adding: boolean;
  keyInput: string;
}

interface Props {
  onClose: () => void;
  onTimezoneChange?: (tz: string) => void;
  onRepoChange?: () => void;
}

// ---------------------------------------------------------------------------
// Repos tab
// ---------------------------------------------------------------------------

const PLATFORM_LABELS: Record<string, string> = {
  twitter: 'X',
  x: 'X',
  bluesky: 'Bluesky',
  mastodon: 'Mastodon',
  linkedin: 'LinkedIn',
};

function platformLabel(platform: string): string {
  return PLATFORM_LABELS[platform] ?? platform;
}

interface ProfileSelectorProps {
  repoId: string;
  /** Bump this to force a reload of profiles (e.g. after per-repo key changes) */
  credentialVersion: number;
}

function ProfileSelector({ repoId, credentialVersion }: ProfileSelectorProps) {
  const [accounts, setAccounts] = useState<SchedulerProfile[]>([]);
  const [selected, setSelected] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Load saved account selections from config.json
    invoke<Record<string, string>>('get_account_ids', { repoId })
      .then((result) => setSelected(result ?? {}))
      .catch(() => {}); // non-fatal — selections just default to empty
  }, [repoId]);

  useEffect(() => {
    setError(null);
    invoke<SchedulerProfile[]>('list_profiles_for_repo', { repoId })
      .then(setAccounts)
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [repoId, credentialVersion]);

  async function handleChange(platform: string, accountId: string) {
    setSelected((prev) => ({ ...prev, [platform]: accountId }));
    try {
      await invoke('save_account_id', { repoId, platform, accountId });
    } catch (e) {
      console.error('save_account_id failed:', e);
    }
  }

  // Group accounts by the platform they serve (each account has platforms[0]).
  const byPlatform = accounts.reduce<Record<string, SchedulerProfile[]>>((acc, profile) => {
    const platform = profile.platforms[0];
    if (platform) {
      acc[platform] = [...(acc[platform] ?? []), profile];
    }
    return acc;
  }, {});

  const platforms = Object.keys(byPlatform);

  return (
    <div className="mt-3 border-t border-zinc-100 pt-3 dark:border-zinc-700">
      <p className="mb-2 text-xs font-medium text-zinc-600 dark:text-zinc-400">
        Posting accounts
      </p>
      {error ? (
        <p className="text-xs text-red-500">{error}</p>
      ) : accounts.length === 0 ? (
        <p className="text-xs text-zinc-400">No accounts connected. Add credentials in Settings → Scheduler.</p>
      ) : (
        <div className="space-y-2">
          {platforms.map((platform) => (
            <div key={platform} className="flex items-center gap-3">
              <span className="w-16 shrink-0 text-xs text-zinc-500">
                {platformLabel(platform)}
              </span>
              <select
                aria-label={`${platformLabel(platform)} account`}
                value={selected[platform] ?? ''}
                onChange={(e) => handleChange(platform, e.target.value)}
                className="flex-1 rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              >
                <option value="">— select account —</option>
                {byPlatform[platform].map((a) => (
                  <option key={a.id} value={a.id}>{a.name}</option>
                ))}
              </select>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

interface RepoSchedulerKeyProps {
  repoId: string;
  provider: string;
  onCredentialChange: () => void;
}

function RepoSchedulerKey({ repoId, provider, onCredentialChange }: RepoSchedulerKeyProps) {
  const [maskedKey, setMaskedKey] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [keyInput, setKeyInput] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const result = await invoke<string | null>('get_scheduler_credential', { provider, repoId });
      setMaskedKey(result ?? null);
    } catch {
      setMaskedKey(null);
    }
  }, [provider, repoId]);

  useEffect(() => { load(); }, [load]);

  async function handleSave() {
    if (!keyInput.trim()) return;
    setSaving(true);
    setSaveError(null);
    try {
      await invoke('save_scheduler_credential', { provider, apiKey: keyInput.trim(), repoId });
      setKeyInput('');
      setAdding(false);
      await load();
      onCredentialChange();
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  async function handleRemove() {
    try {
      await invoke('delete_scheduler_credential', { provider, repoId });
      await load();
      onCredentialChange();
    } catch (e) {
      console.error('delete_scheduler_credential failed:', e);
    }
  }

  const providerLabel = provider.charAt(0).toUpperCase() + provider.slice(1);

  return (
    <div className="mt-3 border-t border-zinc-100 pt-3 dark:border-zinc-700">
      <p className="mb-2 text-xs font-medium text-zinc-600 dark:text-zinc-400">
        {providerLabel} API key
      </p>
      {adding ? (
        <div className="space-y-2">
          <input
            type="password"
            value={keyInput}
            onChange={(e) => setKeyInput(e.target.value)}
            placeholder="Paste API key…"
            className="w-full rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          />
          {saveError && <p className="text-xs text-red-500">{saveError}</p>}
          <div className="flex gap-2">
            <Button onClick={handleSave} disabled={saving || !keyInput.trim()}>
              {saving ? 'Saving…' : 'Save'}
            </Button>
            <Button outline onClick={() => { setAdding(false); setKeyInput(''); setSaveError(null); }}>
              Cancel
            </Button>
          </div>
        </div>
      ) : maskedKey ? (
        <div className="flex items-center gap-3">
          <span className="flex-1 font-mono text-xs text-zinc-500">{maskedKey}</span>
          <Button outline onClick={handleRemove}>Remove override</Button>
        </div>
      ) : (
        <div className="flex items-center gap-3">
          <span className="flex-1 text-xs text-zinc-400">Using global key</span>
          <Button outline onClick={() => setAdding(true)}>Override for this repo</Button>
        </div>
      )}
    </div>
  );
}

function ReposTab({ onRepoChange }: { onRepoChange: () => void }) {
  const [repos, setRepos] = useState<RepoWithStatus[]>([]);
  const [removeConfirmId, setRemoveConfirmId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [actionError, setActionError] = useState<string | null>(null);
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  // Bump per-repo to reload ProfileSelector after credential changes
  const [credentialVersions, setCredentialVersions] = useState<Record<string, number>>({});

  function bumpCredentialVersion(repoId: string) {
    setCredentialVersions((prev) => ({ ...prev, [repoId]: (prev[repoId] ?? 0) + 1 }));
  }

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<RepoWithStatus[]>('get_repos');
      setRepos(result);
    } catch (e) {
      console.error('get_repos failed:', e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  async function handleAdd() {
    setActionError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;
    try {
      await invoke('add_repo', { path: selected });
      refresh();
      onRepoChange();
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to add repo');
    }
  }

  async function handleRemove(id: string) {
    setActionError(null);
    try {
      await invoke('remove_repo', { id });
      setRemoveConfirmId(null);
      refresh();
      onRepoChange();
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to remove repo');
    }
  }

  async function handleToggleActive(id: string, active: boolean) {
    if (togglingIds.has(id)) return;
    setTogglingIds((prev) => new Set(prev).add(id));
    setActionError(null);
    try {
      await invoke('set_repo_active', { id, active });
      refresh();
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to update repo');
    } finally {
      setTogglingIds((prev) => { const next = new Set(prev); next.delete(id); return next; });
    }
  }

  async function handleUpdatePath(id: string) {
    setActionError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;
    try {
      await invoke('update_repo_path', { id, newPath: selected });
      refresh();
    } catch (e) {
      setActionError(e instanceof Error ? e.message : 'Failed to update repo path');
    }
  }

  if (loading) return <p className="text-sm text-zinc-400">Loading…</p>;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Repos</h2>
        <Button onClick={handleAdd}>Add repo</Button>
      </div>

      <div className="space-y-2">
        {repos.map((repo) => {
          const isNotFound = !repo.path_exists;
          return (
            <div
              key={repo.id}
              className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-zinc-900 dark:text-zinc-100">
                      {repo.name}
                    </span>
                    {isNotFound ? (
                      <span title="not found" className="text-yellow-500">⚠</span>
                    ) : repo.active ? (
                      <span title="active" className="text-green-500">●</span>
                    ) : (
                      <span title="inactive" className="text-zinc-400">○</span>
                    )}
                  </div>
                  <p className="mt-0.5 truncate text-xs text-zinc-500">
                    {repo.path}
                    {isNotFound && <span className="ml-1 text-yellow-600">(missing)</span>}
                  </p>
                </div>
                <div className="flex shrink-0 gap-2">
                  {isNotFound ? (
                    <>
                      <Button outline onClick={() => handleUpdatePath(repo.id)}>
                        Update path
                      </Button>
                      <Button outline onClick={() => setRemoveConfirmId(repo.id)}>
                        Remove
                      </Button>
                    </>
                  ) : (
                    <>
                      <Button
                        outline
                        disabled={togglingIds.has(repo.id)}
                        onClick={() => handleToggleActive(repo.id, !repo.active)}
                      >
                        {repo.active ? 'Deactivate' : 'Activate'}
                      </Button>
                      <Button outline onClick={() => setRemoveConfirmId(repo.id)}>
                        Remove
                      </Button>
                    </>
                  )}
                </div>
              </div>
              {!isNotFound && repo.provider && (
                <RepoSchedulerKey
                  repoId={repo.id}
                  provider={repo.provider}
                  onCredentialChange={() => bumpCredentialVersion(repo.id)}
                />
              )}
              {!isNotFound && (
                <ProfileSelector
                  repoId={repo.id}
                  credentialVersion={credentialVersions[repo.id] ?? 0}
                />
              )}
            </div>
          );
        })}

        {repos.length === 0 && (
          <p className="text-sm text-zinc-500">No repos registered. Add one to get started.</p>
        )}
      </div>

      {actionError && (
        <p className="mt-2 text-sm text-red-600 dark:text-red-400">{actionError}</p>
      )}

      {/* Remove confirmation dialog */}
      <Dialog
        open={removeConfirmId !== null}
        onClose={() => setRemoveConfirmId(null)}
      >
        <DialogTitle>Remove repo</DialogTitle>
        <DialogDescription>
          This removes the repo from Postlane. Your files are not affected.
        </DialogDescription>
        <DialogActions>
          <Button plain onClick={() => setRemoveConfirmId(null)}>Cancel</Button>
          <Button color="red" onClick={() => removeConfirmId && handleRemove(removeConfirmId)}>
            Remove
          </Button>
        </DialogActions>
      </Dialog>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Scheduler tab
// ---------------------------------------------------------------------------

function SchedulerTab() {
  const [creds, setCreds] = useState<Record<Provider, CredentialState>>({
    zernio: { preview: null, testing: false, testResult: null, testError: null, adding: false, keyInput: '' },
    buffer: { preview: null, testing: false, testResult: null, testError: null, adding: false, keyInput: '' },
    ayrshare: { preview: null, testing: false, testResult: null, testError: null, adding: false, keyInput: '' },
  });
  const [removeConfirmProvider, setRemoveConfirmProvider] = useState<Provider | null>(null);
  const [removeConfirmInput, setRemoveConfirmInput] = useState('');

  useEffect(() => {
    PROVIDERS.forEach(async (provider) => {
      try {
        const preview = await invoke<string>('get_scheduler_credential', { provider });
        setCreds((prev) => ({
          ...prev,
          [provider]: { ...prev[provider], preview },
        }));
      } catch {
        // not configured
      }
    });
  }, []);

  function update(provider: Provider, patch: Partial<CredentialState>) {
    setCreds((prev) => ({ ...prev, [provider]: { ...prev[provider], ...patch } }));
  }

  async function handleSave(provider: Provider) {
    const key = creds[provider].keyInput;
    if (!key) return;
    try {
      await invoke('save_scheduler_credential', { provider, apiKey: key });
      update(provider, { preview: `••••${key.slice(-4)}`, adding: false, keyInput: '' });
    } catch (e) {
      console.error('save credential failed:', e);
    }
  }

  async function handleRemove(provider: Provider) {
    try {
      await invoke('delete_scheduler_credential', { provider });
      update(provider, { preview: null, testResult: null });
      setRemoveConfirmProvider(null);
      setRemoveConfirmInput('');
    } catch (e) {
      update(provider, {
        testResult: 'error',
        testError: e instanceof Error ? e.message : 'Failed to remove credential',
      });
    }
  }

  function openRemoveConfirm(provider: Provider) {
    setRemoveConfirmInput('');
    setRemoveConfirmProvider(provider);
  }

  function closeRemoveConfirm() {
    setRemoveConfirmProvider(null);
    setRemoveConfirmInput('');
  }

  async function handleTest(provider: Provider) {
    update(provider, { testing: true, testResult: null, testError: null });
    try {
      await invoke('test_scheduler', { provider });
      update(provider, { testing: false, testResult: 'ok' });
    } catch (e) {
      update(provider, {
        testing: false,
        testResult: 'error',
        testError: e instanceof Error ? e.message : 'Test failed',
      });
    }
  }

  return (
    <div className="space-y-4">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Default scheduler</h2>
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        These credentials apply to all repos by default. Per-repo scheduler accounts
        (for separate businesses or clients) will be configurable in v1.1 via
        Settings → Repos → Configure.
      </p>
      <div className="rounded-lg border border-blue-200 bg-blue-50 px-3 py-2.5 text-xs text-blue-800 dark:border-blue-800 dark:bg-blue-950 dark:text-blue-200">
        <strong>macOS Keychain:</strong> When you save an API key, macOS will ask for
        your login password to store it securely. Enter your password and click{' '}
        <strong>Always Allow</strong> — this is a one-time prompt per key.
        Your API keys are stored in Keychain, never in any file on disk.
      </div>
      {PROVIDERS.map((provider) => {
        const c = creds[provider];
        return (
          <div key={provider} className="rounded-lg border border-zinc-200 p-4 dark:border-zinc-700">
            <div className="flex items-center justify-between gap-4">
              <div className="flex items-center gap-3">
                <span className="font-medium capitalize text-zinc-900 dark:text-zinc-100">
                  {provider}
                </span>
                {c.preview ? (
                  <span className="text-xs text-zinc-500">{c.preview}</span>
                ) : (
                  <span className="text-xs text-zinc-400">not configured</span>
                )}
              </div>
              <div className="flex items-center gap-2">
                {c.testResult === 'ok' && <span className="text-xs text-green-600">✓</span>}
                {c.testResult === 'error' && (
                  <span className="text-xs text-red-600">{c.testError}</span>
                )}
                {c.preview ? (
                  <>
                    <Button outline onClick={() => handleTest(provider)} disabled={c.testing}>
                      Test
                    </Button>
                    <Button outline onClick={() => update(provider, { adding: true })}>
                      Change
                    </Button>
                    <Button outline onClick={() => openRemoveConfirm(provider)}>
                      Remove
                    </Button>
                  </>
                ) : (
                  <Button outline onClick={() => update(provider, { adding: true })}>
                    + Add
                  </Button>
                )}
              </div>
            </div>
            {c.adding && (
              <div className="mt-3 flex gap-2">
                <input
                  type="password"
                  value={c.keyInput}
                  onChange={(e) => update(provider, { keyInput: e.target.value })}
                  placeholder="API key"
                  className="flex-1 rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
                />
                <Button onClick={() => handleSave(provider)}>Save</Button>
                <Button plain onClick={() => update(provider, { adding: false, keyInput: '' })}>
                  Cancel
                </Button>
              </div>
            )}
          </div>
        );
      })}
      {/* Type-to-confirm removal dialog */}
      <Dialog open={removeConfirmProvider !== null} onClose={closeRemoveConfirm}>
        <DialogTitle>Remove {removeConfirmProvider} API key</DialogTitle>
        <DialogDescription>
          This will permanently delete the API key from your macOS Keychain.
          Any repos using {removeConfirmProvider} will stop working until a new key is added.
        </DialogDescription>
        <DialogBody>
          <p className="mb-2 text-sm text-zinc-700 dark:text-zinc-300">
            Type <strong>{removeConfirmProvider}</strong> to confirm:
          </p>
          <input
            type="text"
            value={removeConfirmInput}
            onChange={(e) => setRemoveConfirmInput(e.target.value)}
            placeholder={removeConfirmProvider ?? ''}
            className="w-full rounded-lg border border-zinc-300 px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            autoFocus
          />
        </DialogBody>
        <DialogActions>
          <Button plain onClick={closeRemoveConfirm}>Cancel</Button>
          <Button
            color="red"
            disabled={removeConfirmInput !== removeConfirmProvider}
            onClick={() => removeConfirmProvider && handleRemove(removeConfirmProvider)}
          >
            Remove
          </Button>
        </DialogActions>
      </Dialog>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App tab
// ---------------------------------------------------------------------------

// A selection of common IANA timezones — enough for the UI without being overwhelming
const COMMON_TIMEZONES = [
  'UTC',
  'America/New_York',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'America/Sao_Paulo',
  'Europe/London',
  'Europe/Paris',
  'Europe/Berlin',
  'Asia/Dubai',
  'Asia/Kolkata',
  'Asia/Singapore',
  'Asia/Tokyo',
  'Australia/Sydney',
];

function AppTab({ onTimezoneChange }: { onTimezoneChange?: (tz: string) => void }) {
  const currentTimezone = useTimezone();
  const [version, setVersion] = useState('');
  const [autostart, setAutostart] = useState(false);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updateResult, setUpdateResult] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(console.error);
    invoke<boolean>('get_autostart_enabled').then(setAutostart).catch(console.error);
  }, []);

  async function handleTimezoneChange(tz: string) {
    try {
      const appState = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...appState, timezone: tz } });
      onTimezoneChange?.(tz);
    } catch (e) {
      console.error('Failed to save timezone:', e);
    }
  }

  async function handleAutostartToggle() {
    try {
      if (autostart) {
        await invoke('plugin:autostart|disable');
      } else {
        await invoke('plugin:autostart|enable');
      }
      setAutostart(!autostart);
    } catch (e) {
      console.error('autostart toggle failed:', e);
    }
  }

  async function handleOpenLogs() {
    try {
      await invoke('plugin:opener|open_path', { path: '~/Library/Logs/postlane' });
    } catch (e) {
      console.error('open logs failed:', e);
    }
  }

  async function handleCheckUpdates() {
    setCheckingUpdates(true);
    setUpdateResult(null);
    try {
      const result = await invoke<string | null>('plugin:updater|check');
      setUpdateResult(result ? `Update available: ${result}` : 'You are up to date.');
    } catch (e) {
      setUpdateResult('Could not check for updates.');
    } finally {
      setCheckingUpdates(false);
    }
  }

  return (
    <div className="space-y-6">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">App</h2>

      {/* Launch at login */}
      <div className="flex items-center justify-between">
        <label htmlFor="autostart" className="text-sm text-zinc-700 dark:text-zinc-300">
          Launch at login
        </label>
        <input
          id="autostart"
          type="checkbox"
          role="checkbox"
          aria-label="Launch at login"
          checked={autostart}
          onChange={handleAutostartToggle}
          className="h-4 w-4 rounded border-zinc-300"
        />
      </div>

      {/* Timezone */}
      <div className="flex items-center justify-between">
        <label htmlFor="timezone" className="text-sm text-zinc-700 dark:text-zinc-300">
          Display timezone
        </label>
        <select
          id="timezone"
          value={currentTimezone}
          onChange={(e) => handleTimezoneChange(e.target.value)}
          className="rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
        >
          <option value="">System default</option>
          {COMMON_TIMEZONES.map((tz) => (
            <option key={tz} value={tz}>{tz}</option>
          ))}
        </select>
      </div>

      {/* Logs */}
      <div className="flex items-center justify-between">
        <span className="text-sm text-zinc-700 dark:text-zinc-300">Logs</span>
        <Button outline onClick={handleOpenLogs}>Open log folder →</Button>
      </div>

      {/* Version + updates */}
      <div className="flex items-center justify-between">
        <span className="text-sm text-zinc-700 dark:text-zinc-300">
          Postlane {version}
        </span>
        <div className="flex items-center gap-3">
          {updateResult && (
            <span className="text-xs text-zinc-500">{updateResult}</span>
          )}
          <Button outline onClick={handleCheckUpdates} disabled={checkingUpdates}>
            {checkingUpdates ? 'Checking…' : 'Check for updates'}
          </Button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main panel
// ---------------------------------------------------------------------------

export default function SettingsPanel({ onClose, onTimezoneChange, onRepoChange }: Props) {
  const [activeTab, setActiveTab] = useState<Tab>('repos');

  return (
    <div className="flex h-full flex-col bg-white dark:bg-zinc-900">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-200 px-6 py-4 dark:border-zinc-700">
        <h1 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">Settings</h1>
        <Button plain onClick={onClose} aria-label="Close settings">✕</Button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-zinc-200 px-6 dark:border-zinc-700" role="tablist">
        {(['repos', 'scheduler', 'app'] as Tab[]).map((tab) => (
          <button
            key={tab}
            role="tab"
            aria-selected={activeTab === tab}
            onClick={() => setActiveTab(tab)}
            className={[
              'px-4 py-3 text-sm font-medium border-b-2 -mb-px capitalize focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500',
              activeTab === tab
                ? 'border-zinc-900 text-zinc-900 dark:border-zinc-100 dark:text-zinc-100'
                : 'border-transparent text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300',
            ].join(' ')}
          >
            {tab}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-6">
        {activeTab === 'repos' && <ReposTab onRepoChange={() => onRepoChange?.()} />}
        {activeTab === 'scheduler' && <SchedulerTab />}
        {activeTab === 'app' && <AppTab onTimezoneChange={onTimezoneChange} />}
      </div>
    </div>
  );
}
