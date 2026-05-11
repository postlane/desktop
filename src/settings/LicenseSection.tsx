// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
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
    let url = 'https://postlane.dev/login?desktop=1';
    try {
      const port = await invoke<number>('get_local_server_port');
      url = `https://postlane.dev/login?desktop=1&port=${port}`;
    } catch (e) {
      console.error('[sign-in] get_local_server_port failed — opening without port:', e);
    }
    await openUrl(url).catch(console.error);
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
