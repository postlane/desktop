// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '../ipc/invoke';
import { useProjectsContext } from '../context/ProjectsProvider';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import AccountDangerZone from './AccountDangerZone';

interface Props {
  onSignedOut: () => void;
}

export default function AccountSettingsView({ onSignedOut }: Props) {
  const [displayName, setDisplayName] = useState<string | null>(null);
  const [email, setEmail] = useState<string | null>(null);
  const [signOutLoading, setSignOutLoading] = useState(false);
  const { clear: clearProjects } = useProjectsContext();
  const { clear: clearDrafts } = useDraftPostsContext();

  useEffect(() => {
    invoke<string | null>('get_license_display_name').then(setDisplayName).catch(() => {});
    invoke<string | null>('get_license_email').then(setEmail).catch(() => {});
  }, []);

  async function handleSignOut() {
    setSignOutLoading(true);
    try {
      await invoke('sign_out');
      clearProjects();
      clearDrafts();
      onSignedOut();
    } finally {
      setSignOutLoading(false);
    }
  }

  return (
    <div className="px-5 py-4" style={{ maxWidth: '36rem' }}>
      <p className="is-size-5 has-text-weight-semibold mb-5">Account</p>
      <div className="mb-4">
        <p className="is-size-7 has-text-grey mb-1">Signed in as</p>
        <p className="is-size-7">{displayName ?? 'Signed in'}</p>
      </div>
      <div className="is-flex" style={{ gap: '0.75rem' }}>
        <button className="button is-small is-light" onClick={() => openUrl('https://postlane.dev/account')}>
          Manage account
        </button>
        <button className="button is-small is-light has-text-danger" onClick={handleSignOut} disabled={signOutLoading}>
          Sign out
        </button>
      </div>
      {(email || displayName) && (
        <AccountDangerZone userEmail={email ?? displayName ?? ''} onDeleted={onSignedOut} />
      )}
    </div>
  );
}
