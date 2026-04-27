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
    <div className="space-y-3">
      {expired && (
        <p role="alert" className="text-sm text-red-700 dark:text-red-400">
          Your Postlane license has expired. Sign in at postlane.dev/login.
        </p>
      )}
      {banner && (
        <p role="status" className="text-sm text-green-700 dark:text-green-400">
          {banner}
        </p>
      )}
      {!signedIn && (
        <div className="flex items-center justify-between">
          <span className="text-sm text-zinc-700 dark:text-zinc-300">Account</span>
          <button
            type="button"
            onClick={handleSignIn}
            className="rounded-md bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
          >
            Sign in at postlane.dev
          </button>
        </div>
      )}
    </div>
  );
}
