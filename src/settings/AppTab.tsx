// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone } from '../TimezoneContext';
import type { AppStateFile } from '../types';
import { Button } from '../components/catalyst/button';

const COMMON_TIMEZONES = [
  'UTC', 'America/New_York', 'America/Chicago', 'America/Denver', 'America/Los_Angeles',
  'America/Sao_Paulo', 'Europe/London', 'Europe/Paris', 'Europe/Berlin',
  'Asia/Dubai', 'Asia/Kolkata', 'Asia/Singapore', 'Asia/Tokyo', 'Australia/Sydney',
];

interface AppTabViewProps {
  version: string;
  autostart: boolean;
  checkingUpdates: boolean;
  updateResult: string | null;
  attribution: boolean;
  currentTimezone: string;
  onAttributionToggle: () => void;
  onTimezoneChange: (_tz: string) => void;
  onAutostartToggle: () => void;
  onOpenLogs: () => void;
  onCheckUpdates: () => void;
}

function AppTabView({ version, autostart, checkingUpdates, updateResult, attribution, currentTimezone, onAttributionToggle, onTimezoneChange, onAutostartToggle, onOpenLogs, onCheckUpdates }: AppTabViewProps) {
  return (
    <div className="space-y-6">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">App</h2>
      <div className="flex items-center justify-between">
        <div>
          <span className="text-sm text-zinc-700 dark:text-zinc-300">Post attribution</span>
          <p className="text-xs text-zinc-500 dark:text-zinc-400">Append '📮 postlane.dev' to posts created with Postlane. Opt out at any time.</p>
        </div>
        <button role="switch" aria-label="Post attribution" aria-checked={attribution} onClick={onAttributionToggle}
          className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 ${attribution ? 'bg-blue-600' : 'bg-zinc-300 dark:bg-zinc-600'}`}>
          <span className={`inline-block h-4 w-4 transform rounded-full bg-white shadow transition-transform ${attribution ? 'translate-x-6' : 'translate-x-1'}`} />
        </button>
      </div>
      <div className="flex items-center justify-between">
        <label htmlFor="autostart" className="text-sm text-zinc-700 dark:text-zinc-300">Launch at login</label>
        <input id="autostart" type="checkbox" role="checkbox" aria-label="Launch at login" checked={autostart} onChange={onAutostartToggle} className="h-4 w-4 rounded border-zinc-300" />
      </div>
      <div className="flex items-center justify-between">
        <label htmlFor="timezone" className="text-sm text-zinc-700 dark:text-zinc-300">Display timezone</label>
        <select id="timezone" value={currentTimezone} onChange={(e) => onTimezoneChange(e.target.value)}
          className="rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500">
          <option value="">System default</option>
          {COMMON_TIMEZONES.map((tz) => <option key={tz} value={tz}>{tz}</option>)}
        </select>
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm text-zinc-700 dark:text-zinc-300">Logs</span>
        <Button outline onClick={onOpenLogs}>Open log folder →</Button>
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm text-zinc-700 dark:text-zinc-300">Postlane {version}</span>
        <div className="flex items-center gap-3">
          {updateResult && <span className="text-xs text-zinc-500">{updateResult}</span>}
          <Button outline onClick={onCheckUpdates} disabled={checkingUpdates}>
            {checkingUpdates ? 'Checking…' : 'Check for updates'}
          </Button>
        </div>
      </div>
    </div>
  );
}

export default function AppTab({ onTimezoneChange }: { onTimezoneChange?: (_tz: string) => void }) {
  const currentTimezone = useTimezone();
  const [version, setVersion] = useState('');
  const [autostart, setAutostart] = useState(false);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updateResult, setUpdateResult] = useState<string | null>(null);
  const [attribution, setAttribution] = useState(true);

  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(console.error);
    invoke<boolean>('get_autostart_enabled').then(setAutostart).catch(console.error);
    invoke<boolean>('get_attribution').then(setAttribution).catch(console.error);
  }, []);

  async function handleAttributionToggle() {
    const next = !attribution;
    try { await invoke('set_attribution', { enabled: next }); setAttribution(next); }
    catch (e) { console.error('set_attribution failed:', e); }
  }

  async function handleTimezoneChange(tz: string) {
    try {
      const appState = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...appState, timezone: tz } });
      onTimezoneChange?.(tz);
    } catch (e) { console.error('Failed to save timezone:', e); }
  }

  async function handleAutostartToggle() {
    try {
      await invoke(autostart ? 'plugin:autostart|disable' : 'plugin:autostart|enable');
      setAutostart(!autostart);
    } catch (e) { console.error('autostart toggle failed:', e); }
  }

  async function handleOpenLogs() {
    try { await invoke('plugin:opener|open_path', { path: '~/Library/Logs/postlane' }); }
    catch (e) { console.error('open logs failed:', e); }
  }

  async function handleCheckUpdates() {
    setCheckingUpdates(true);
    setUpdateResult(null);
    try {
      const result = await invoke<string | null>('plugin:updater|check');
      setUpdateResult(result ? `Update available: ${result}` : 'You are up to date.');
    } catch { setUpdateResult('Could not check for updates.'); }
    finally { setCheckingUpdates(false); }
  }

  return (
    <AppTabView
      version={version} autostart={autostart} checkingUpdates={checkingUpdates}
      updateResult={updateResult} attribution={attribution} currentTimezone={currentTimezone}
      onAttributionToggle={handleAttributionToggle} onTimezoneChange={handleTimezoneChange}
      onAutostartToggle={handleAutostartToggle} onOpenLogs={handleOpenLogs}
      onCheckUpdates={handleCheckUpdates}
    />
  );
}
