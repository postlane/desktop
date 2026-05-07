// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';

interface ActivatedEvent {
  display_name: string;
}

export function LicenseSection() {
  const [signedIn, setSignedIn] = useState<boolean | null>(null);
  const [banner, setBanner] = useState<string | null>(null);
  const [expired, setExpired] = useState(false);

  useEffect(() => {
    invoke<boolean>('get_license_signed_in')
      .then(setSignedIn)
      .catch(console.error);

    const unlistenActivated = listen<ActivatedEvent>('license:activated', (event) => {
      setSignedIn(true);
      setBanner(`Postlane activated. Signed in as ${event.payload.display_name}.`);
    });

    const unlistenExpired = listen('license:expired', () => {
      setExpired(true);
    });

    return () => {
      unlistenActivated.then((unlisten) => unlisten());
      unlistenExpired.then((unlisten) => unlisten());
    };
  }, []);

  async function handleSignIn() {
    try {
      await openUrl('https://postlane.dev/login');
    } catch (e) {
      console.error('Failed to open sign-in page:', e);
    }
  }

  if (signedIn === null) return null;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      {expired && (
        <p role="alert" className="is-size-7 has-text-danger">
          Your Postlane license has expired. Sign in at postlane.dev/login.
        </p>
      )}
      {banner && (
        <p role="status" className="is-size-7 has-text-success">
          {banner}
        </p>
      )}
      {!signedIn && (
        <div className="is-flex is-align-items-center is-justify-content-space-between">
          <span className="is-size-7">Account</span>
          <button type="button" onClick={handleSignIn} className="button is-primary is-small">
            Sign in at postlane.dev
          </button>
        </div>
      )}
    </div>
  );
}
