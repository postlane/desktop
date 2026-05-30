// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useTimezone, getTimezoneOffsetLabel } from '../TimezoneContext';
import type { AppStateFile } from '../types';
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
  onCopyLogPath: () => void;
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
  const hourValue = defaultPostTime !== null ? String(defaultPostTime.hour) : '';
  const minuteValue = defaultPostTime !== null ? String(defaultPostTime.minute) : '';
  return (
    <div className="is-flex is-align-items-center is-justify-content-space-between">
      <span className="is-size-7">Default post time</span>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <div className="select is-small">
          <select aria-label="Default post time hour" value={hourValue} onChange={(e) => onHourChange(e.target.value)}>
            <option value="">--</option>
            {Array.from({ length: 24 }, (_, i) => <option key={i} value={String(i)}>{String(i).padStart(2, '0')}</option>)}
          </select>
        </div>
        <div className="select is-small">
          <select aria-label="Default post time minute" value={minuteValue} onChange={(e) => onMinuteChange(e.target.value)}>
            <option value="">--</option>
            {MINUTES.map((m) => <option key={m} value={String(m)}>{String(m).padStart(2, '0')}</option>)}
          </select>
        </div>
        {timezoneLabel && <span className="is-size-7 has-text-grey">({timezoneLabel})</span>}
        {defaultPostTime !== null && (
          <button aria-label="Clear default post time" onClick={onClear} className="button is-ghost is-small has-text-grey-light">Clear</button>
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
    if (!value) { void saveDefaultPostTime(null); return; }
    const hour = parseInt(value, 10);
    if (isNaN(hour)) return;
    const minute = defaultPostTime?.minute ?? 0;
    void saveDefaultPostTime({ hour, minute, timezone: tz });
  }

  function handleMinuteChange(value: string) {
    if (!value) { void saveDefaultPostTime(null); return; }
    const minute = parseInt(value, 10);
    if (isNaN(minute)) return;
    const hour = defaultPostTime?.hour ?? 0;
    void saveDefaultPostTime({ hour, minute, timezone: tz });
  }

  function handleClear() { void saveDefaultPostTime(null); }

  return { defaultPostTime, handleHourChange, handleMinuteChange, handleClear };
}

function AppTabView({ version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent, currentTimezone, defaultPostTime, timezoneLabel, settingsError, onAttributionToggle, onTelemetryToggle, onTimezoneChange, onAutostartToggle, onOpenLogs, onCopyLogPath, onCheckUpdates, onHourChange, onMinuteChange, onClearDefaultPostTime }: AppTabViewProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
      <h2 className="has-text-weight-semibold is-size-7">App</h2>
      {settingsError && <p role="alert" className="is-size-7 has-text-danger">{settingsError}</p>}
      <LicenseSection />
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <div>
          <span className="is-size-7">Post attribution</span>
          <p className="is-size-7 has-text-grey">Append '📮 postlane.dev' to posts created with Postlane. Opt out at any time.</p>
        </div>
        <button role="switch" aria-label="Post attribution" aria-checked={attribution} onClick={onAttributionToggle}
          className={`button is-ghost ${attribution ? 'has-text-primary' : 'has-text-grey-light'}`}
          style={{ width: '2.75rem', height: '1.5rem', borderRadius: '0.75rem', padding: 0, position: 'relative', background: attribution ? '#3273dc' : '#b5b5b5' }}>
          <span style={{ position: 'absolute', top: '0.2rem', left: attribution ? '1.2rem' : '0.2rem', width: '1.1rem', height: '1.1rem', borderRadius: '50%', background: 'white', transition: 'left 0.2s' }} />
        </button>
      </div>
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <div>
          <label htmlFor="telemetry" className="is-size-7">Send anonymous usage data</label>
          <p className="is-size-7 has-text-grey">Which skills you use, post approvals, scheduler used. No post content.</p>
          <a href="https://postlane.dev/docs/privacy" target="_blank" rel="noreferrer" className="is-size-7 has-text-link">What data is sent? →</a>
        </div>
        <input id="telemetry" type="checkbox" role="checkbox" aria-label="Send anonymous usage data"
          checked={telemetryConsent} onChange={onTelemetryToggle} className="checkbox" />
      </div>
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <label htmlFor="autostart" className="is-size-7">Launch at login</label>
        <input id="autostart" type="checkbox" role="checkbox" aria-label="Launch at login" checked={autostart} onChange={onAutostartToggle} className="checkbox" />
      </div>
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <label htmlFor="timezone" className="is-size-7">Display timezone</label>
        <div className="select is-small">
          <select id="timezone" value={currentTimezone} onChange={(e) => onTimezoneChange(e.target.value)}>
            <option value="">System default</option>
            {COMMON_TIMEZONES.map((tz) => <option key={tz} value={tz}>{tz}</option>)}
          </select>
        </div>
      </div>
      <DefaultPostTimeRow defaultPostTime={defaultPostTime} timezoneLabel={timezoneLabel} onHourChange={onHourChange} onMinuteChange={onMinuteChange} onClear={onClearDefaultPostTime} />
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <span className="is-size-7">Logs</span>
        <div className="is-flex" style={{ gap: '0.5rem' }}>
          <button className="button is-outlined is-small" onClick={onOpenLogs}>Open log folder →</button>
          <button className="button is-outlined is-small" data-testid="copy-log-path" onClick={onCopyLogPath}>Copy log path</button>
        </div>
      </div>
      <div className="is-flex is-align-items-center is-justify-content-space-between">
        <span className="is-size-7">Postlane {version}</span>
        <div className="is-flex is-align-items-center" style={{ gap: '0.75rem' }}>
          {updateResult && <span className="is-size-7 has-text-grey">{updateResult}</span>}
          <button className="button is-outlined is-small" onClick={onCheckUpdates} disabled={checkingUpdates}>
            {checkingUpdates ? 'Checking…' : 'Check for updates'}
          </button>
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

  async function handleCopyLogPath() {
    try { const p = await invoke<string>('get_log_path'); await invoke('plugin:clipboard-manager|write_text', { text: p }); }
    catch (e) { handleIpcError(e); }
  }

  return {
    version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent,
    currentTimezone, timezoneLabel: getTimezoneOffsetLabel(currentTimezone),
    settingsError,
    handleAttributionToggle, handleTelemetryToggle, handleTimezoneChange,
    handleAutostartToggle, handleOpenLogs, handleCopyLogPath, handleCheckUpdates,
    defaultPostTime, handleHourChange, handleMinuteChange, handleClear,
  };
}

export default function AppTab({ onTimezoneChange }: { onTimezoneChange?: (_tz: string) => void }) {
  const {
    version, autostart, checkingUpdates, updateResult, attribution, telemetryConsent,
    currentTimezone, timezoneLabel, settingsError, handleAttributionToggle, handleTelemetryToggle,
    handleTimezoneChange, handleAutostartToggle, handleOpenLogs, handleCopyLogPath, handleCheckUpdates,
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
      onOpenLogs={handleOpenLogs} onCopyLogPath={handleCopyLogPath} onCheckUpdates={handleCheckUpdates}
      onHourChange={handleHourChange} onMinuteChange={handleMinuteChange}
      onClearDefaultPostTime={handleClear}
    />
  );
}
