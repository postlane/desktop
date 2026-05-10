// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import type { AppStateFile } from '../types';

// ── Constants ─────────────────────────────────────────────────────────────────

const IS_MAC = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform);
const TIMEZONES: string[] = typeof Intl !== 'undefined' && 'supportedValuesOf' in Intl
  ? (Intl as unknown as { supportedValuesOf: (_k: string) => string[] }).supportedValuesOf('timeZone')
  : [];

// ── Types ─────────────────────────────────────────────────────────────────────

interface Props {
  onTimezoneChange: (_tz: string) => void;
}

// ── Static preference fields ──────────────────────────────────────────────────

function StaticFields() {
  return (
    <>
      <div className="field mb-4">
        <label className="label is-small">Theme</label>
        <div className="select is-small" title="Coming soon">
          <select disabled>
            <option>System default</option>
          </select>
        </div>
      </div>
      {IS_MAC && (
        <div className="field mb-4">
          <label className="checkbox is-size-7">
            <input type="checkbox" aria-label="Launch at login" style={{ marginRight: '0.5rem' }} />
            Launch at login
          </label>
        </div>
      )}
    </>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function PreferencesSettingsView({ onTimezoneChange }: Props) {
  const [appState, setAppState] = useState<AppStateFile | null>(null);

  useEffect(() => {
    invoke<AppStateFile>('get_app_state').then(setAppState).catch(console.error);
  }, []);

  async function save(patch: Partial<AppStateFile>) {
    if (!appState) return;
    const updated = { ...appState, ...patch };
    setAppState(updated);
    await invoke('save_app_state_command', { state: updated });
  }

  async function handleTimezoneChange(tz: string) {
    await save({ timezone: tz });
    onTimezoneChange(tz);
  }

  async function handleNotificationsToggle() {
    const current = appState?.notifications_enabled ?? true;
    await save({ notifications_enabled: !current });
  }

  const notificationsEnabled = appState?.notifications_enabled ?? true;

  return (
    <div className="px-5 py-4" style={{ maxWidth: '36rem' }}>
      <p className="is-size-5 has-text-weight-semibold mb-5">Preferences</p>
      <div className="field mb-4">
        <label className="label is-small" htmlFor="pref-timezone">Timezone</label>
        <div className="control">
          <div className="select is-small is-fullwidth">
            <select id="pref-timezone" aria-label="Timezone"
              value={appState?.timezone ?? ''}
              onChange={(e) => handleTimezoneChange(e.target.value)}>
              {TIMEZONES.map((tz) => <option key={tz} value={tz}>{tz}</option>)}
            </select>
          </div>
        </div>
      </div>
      <div className="field mb-4">
        <label className="checkbox is-size-7">
          <input id="pref-notifications" type="checkbox" checked={notificationsEnabled}
            aria-label="Notifications enabled"
            onChange={handleNotificationsToggle} style={{ marginRight: '0.5rem' }} />
          Enable desktop notifications
        </label>
      </div>
      <StaticFields />
    </div>
  );
}
