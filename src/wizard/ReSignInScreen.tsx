// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';

interface Props {
  onSignedIn: () => void;
  pollIntervalMs?: number;
}

export default function ReSignInScreen({ onSignedIn, pollIntervalMs = 2000 }: Props) {
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const signed = await invoke<boolean>('get_license_signed_in');
        if (signed) {
          clearInterval(interval);
          onSignedIn();
        }
      } catch { /* ignore poll errors */ }
    }, pollIntervalMs);
    return () => clearInterval(interval);
  }, [onSignedIn, pollIntervalMs]);

  function handleProvider(provider: string) {
    openUrl(`https://postlane.dev/login?provider=${provider}`).catch(console.error);
  }

  return (
    <div style={{ position: 'fixed', inset: 0 }} className="has-background-white is-flex is-align-items-center is-justify-content-center">
      <div style={{ maxWidth: 360, width: '100%', padding: '0 24px' }}>
        <h1 className="title is-4 mb-2">Sign in to Postlane</h1>
        <p className="has-text-grey mb-5">Your session has expired. Sign in again to continue.</p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <button className="button is-dark is-fullwidth" onClick={() => handleProvider('github')}>
            Continue with GitHub
          </button>
          <button
            className="button is-fullwidth"
            style={{ borderColor: '#fc6d26', color: '#fc6d26' }}
            onClick={() => handleProvider('gitlab')}
          >
            Continue with GitLab
          </button>
        </div>
      </div>
    </div>
  );
}
