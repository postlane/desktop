// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import type { AppStateFile, Project } from '../types';

interface Props {
  project: Project;
  onConnect: () => void;
}

export default function OrgUpgradeBanner({ project, onConnect }: Props) {
  const [dismissed, setDismissed] = useState<boolean | null>(null);

  useEffect(() => {
    invoke<AppStateFile>('read_app_state_command')
      .then((state) => { setDismissed(state.org_upgrade_banner_dismissed_v1_2 ?? false); })
      .catch(() => { setDismissed(false); });
  }, []);

  async function handleDismiss() {
    try {
      const state = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...state, org_upgrade_banner_dismissed_v1_2: true } });
    } finally {
      setDismissed(true);
    }
  }

  if (!project.is_owner || project.provider_org_login || dismissed === null || dismissed === true) {
    return null;
  }

  return (
    <div role="alert" className="notification is-info is-light mx-3 my-2 py-2 px-3">
      <p className="is-size-7">
        Connect your GitHub org to unlock the new billing view.
      </p>
      <div className="mt-2" style={{ display: 'flex', gap: '0.5rem' }}>
        <button className="button is-small is-info" onClick={onConnect}>
          Connect org
        </button>
        <button className="button is-small" onClick={handleDismiss}>
          Dismiss
        </button>
      </div>
    </div>
  );
}
