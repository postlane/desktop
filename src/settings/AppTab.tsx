// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTimezone, getTimezoneOffsetLabel } from '../TimezoneContext';
import type { AppStateFile } from '../types';
import { Button } from '../components/catalyst/button';
import { LicenseSection } from './LicenseSection';

type DefaultPostTime = { hour: number; minute: number; timezone: string } | null;

const MINUTES = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];

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
  telemetryConsent: boolean;
  currentTimezone: string;
  defaultPostTime: DefaultPostTime;
  timezoneLabel: string;
  settingsError: string | null;
  onAttributionToggle: () => void;
  onTelemetryToggle: () => void;
  onTimezoneChange: (_tz: string) => void;
  onAutostartToggle: () => void;
  onOpenLogs: () => void;
  onCheckUpdates: () => void;
  onHourChange: (_h: string) => void;
  onMinuteChange: (_m: string) => void;
  onClearDefaultPostTime: () => void;
}

function DefaultPostTimeRow({ defaultPostTime, timezoneLabel, onHourChange, onMinuteChange, onClear }: {
  defaultPostTime: DefaultPostTime;
  timezoneLabel: string;
  onHourChange: (_h: string) => void;
  onMinuteChange: (_m: string) => void;
  onClear: () => void;
}) {
  const selectClass = 'rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500';
  const hourValue = defaultPostTime !== null ? String(defaultPostTime.hour) : '';
  const minuteValue = defaultPostTime !== null ? String(defaultPostTime.minute) : '';
  return (
    <div className="flex items-center justify-between">
      <span className="text-sm text-zinc-700 dark:text-zinc-300">Default post time</span>
      <div className="flex items-center gap-2">
        <select aria-label="Default post time hour" value={hourValue} onChange={(e) => onHourChange(e.target.value)} className={selectClass}>
          <option value="">--</option>
          {Array.from({ length: 24 }, (_, i) => <option key={i} value={String(i)}>{String(i).padStart(2, '0')}</option>)}
        </select>
        <select aria-label="Default post time minute" value={minuteValue} onChange={(e) => onMinuteChange(e.target.value)} className={selectClass}>
          <option value="">--</option>
          {MINUTES.map((m) => <option key={m} value={String(m)}>{String(m).padStart(2, '0')}</option>)}
        </select>
        {timezoneLabel && (
          <span className="text-xs text-zinc-500 dark:text-zinc-400">({timezoneLabel})</span>
        )}
        {defaultPostTime !== null && (
          <button aria-label="Clear default post time" onClick={onClear} className="text-xs text-zinc-400 hover:text-red-500 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500">Clear</button>
        )}
      </div>
    </div>
  );
}

function useDefaultPostTime() {
  const tz = useTimezone();
  const [defaultPostTime, setDefaultPostTime] = useState<DefaultPostTime>(null);

  useEffect(() => {
    invoke<AppStateFile>('read_app_state_command')
      .then((s) => setDefaultPostTime(s.default_post_time ?? null))
      .catch(console.error);
  }, []);

  async function saveDefaultPostTime(dpt: DefaultPostTime) {
    try {
      await invoke('set_default_post_time', { dpt });
      setDefaultPostTime(dpt);
    } catch (e) { console.error('Failed to save default post time:', e); }
  }

  function handleHourChange(value: string) {
    const hour = parseInt(value, 10);
    const minute = defaultPostTime?.minute ?? 0;
    void saveDefaultPostTime({ hour, minute, timezone: tz });
  }

  function handleMinuteChange(value: string) {
    const minute = parseInt(value, 10);
    const hour = defaultPostTime?.hour ?? 0;
    void saveDefaultPostTime({ hour, minute, timezone: tz });
  }

  function handleClear() { void saveDefaultPostTime(null); }

  return { defaultPostTime, handleHourChange, handleMinuteChange, handleClear };
}

function AppTabView({ version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent, currentTimezone, defaultPostTime, timezoneLabel, settingsError, onAttributionToggle, onTelemetryToggle, onTimezoneChange, onAutostartToggle, onOpenLogs, onCheckUpdates, onHourChange, onMinuteChange, onClearDefaultPostTime }: AppTabViewProps) {
  return (
    <div className="space-y-6">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">App</h2>
      {settingsError && <p role="alert" className="text-xs text-red-600 dark:text-red-400">{settingsError}</p>}
      <LicenseSection />
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
        <div>
          <label htmlFor="telemetry" className="text-sm text-zinc-700 dark:text-zinc-300">Send anonymous usage data</label>
          <p className="text-xs text-zinc-500 dark:text-zinc-400">Which skills you use, post approvals, scheduler used. No post content.</p>
          <a href="https://postlane.dev/docs/privacy" target="_blank" rel="noreferrer" className="text-xs text-blue-600 hover:underline dark:text-blue-400">What data is sent? →</a>
        </div>
        <input id="telemetry" type="checkbox" role="checkbox" aria-label="Send anonymous usage data"
          checked={telemetryConsent} onChange={onTelemetryToggle} className="h-4 w-4 rounded border-zinc-300" />
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
      <DefaultPostTimeRow defaultPostTime={defaultPostTime} timezoneLabel={timezoneLabel} onHourChange={onHourChange} onMinuteChange={onMinuteChange} onClear={onClearDefaultPostTime} />
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

function useUpdateCheck() {
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updateResult, setUpdateResult] = useState<string | null>(null);

  async function handleCheckUpdates() {
    setCheckingUpdates(true); setUpdateResult(null);
    try {
      const result = await invoke<string | null>('plugin:updater|check');
      setUpdateResult(result ? `Update available: ${result}` : 'You are up to date.');
    } catch { setUpdateResult('Could not check for updates.'); }
    finally { setCheckingUpdates(false); }
  }

  return { checkingUpdates, updateResult, handleCheckUpdates };
}

function useAppTab(onTimezoneChange?: (_tz: string) => void) {
  const currentTimezone = useTimezone();
  const [version, setVersion] = useState('');
  const [autostart, setAutostart] = useState(false);
  const [attribution, setAttribution] = useState(true);
  const [telemetryConsent, setTelemetryConsent] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const { checkingUpdates, updateResult, handleCheckUpdates } = useUpdateCheck();
  const { defaultPostTime, handleHourChange, handleMinuteChange, handleClear } = useDefaultPostTime();

  useEffect(() => {
    invoke<string>('get_app_version').then(setVersion).catch(console.error);
    invoke<boolean>('get_autostart_enabled').then(setAutostart).catch(console.error);
    invoke<boolean>('get_attribution').then(setAttribution).catch(console.error);
    invoke<boolean>('get_telemetry_consent').then(setTelemetryConsent).catch(console.error);
  }, []);

  function handleIpcError(e: unknown) {
    setSettingsError(e instanceof Error ? e.message : 'Settings could not be saved');
  }

  async function handleAttributionToggle() {
    const next = !attribution;
    try { setSettingsError(null); await invoke('set_attribution', { enabled: next }); setAttribution(next); }
    catch (e) { handleIpcError(e); }
  }

  async function handleTelemetryToggle() {
    const next = !telemetryConsent;
    try { setSettingsError(null); await invoke('set_telemetry_consent', { consent: next }); setTelemetryConsent(next); }
    catch (e) { handleIpcError(e); }
  }

  async function handleTimezoneChange(tz: string) {
    try {
      setSettingsError(null);
      const appState = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...appState, timezone: tz } });
      onTimezoneChange?.(tz);
    } catch (e) { handleIpcError(e); }
  }

  async function handleAutostartToggle() {
    try {
      setSettingsError(null);
      await invoke(autostart ? 'plugin:autostart|disable' : 'plugin:autostart|enable');
      setAutostart(!autostart);
    } catch (e) { handleIpcError(e); }
  }

  async function handleOpenLogs() {
    try { await invoke('plugin:opener|open_path', { path: '~/Library/Logs/postlane' }); }
    catch (e) { handleIpcError(e); }
  }

  return {
    version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent,
    currentTimezone, timezoneLabel: getTimezoneOffsetLabel(currentTimezone),
    settingsError,
    handleAttributionToggle, handleTelemetryToggle, handleTimezoneChange,
    handleAutostartToggle, handleOpenLogs, handleCheckUpdates,
    defaultPostTime, handleHourChange, handleMinuteChange, handleClear,
  };
}

export default function AppTab({ onTimezoneChange }: { onTimezoneChange?: (_tz: string) => void }) {
  const {
    version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent,
    currentTimezone, timezoneLabel, settingsError, handleAttributionToggle, handleTelemetryToggle,
    handleTimezoneChange, handleAutostartToggle, handleOpenLogs, handleCheckUpdates,
    defaultPostTime, handleHourChange, handleMinuteChange, handleClear,
  } = useAppTab(onTimezoneChange);
  return (
    <AppTabView
      version={version} autostart={autostart} checkingUpdates={checkingUpdates}
      updateResult={updateResult} attribution={attribution} telemetryConsent={telemetryConsent}
      currentTimezone={currentTimezone} defaultPostTime={defaultPostTime} timezoneLabel={timezoneLabel}
      settingsError={settingsError}
      onAttributionToggle={handleAttributionToggle} onTelemetryToggle={handleTelemetryToggle}
      onTimezoneChange={handleTimezoneChange} onAutostartToggle={handleAutostartToggle}
      onOpenLogs={handleOpenLogs} onCheckUpdates={handleCheckUpdates}
      onHourChange={handleHourChange} onMinuteChange={handleMinuteChange}
      onClearDefaultPostTime={handleClear}
    />
  );
}
