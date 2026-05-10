// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import type { AppStateFile } from '../types';

export default function UnassignedDraftBanner() {
  const { drafts } = useDraftPostsContext();
  const [dismissed, setDismissed] = useState<boolean | null>(null);

  useEffect(() => {
    invoke<AppStateFile>('read_app_state_command')
      .then((state) => { setDismissed(state.dismissed_unassigned_draft_warning ?? false); })
      .catch(() => { setDismissed(false); });
  }, []);

  async function handleDismiss() {
    try {
      const state = await invoke<AppStateFile>('read_app_state_command');
      await invoke('save_app_state_command', { state: { ...state, dismissed_unassigned_draft_warning: true } });
    } finally {
      setDismissed(true);
    }
  }

  const hasUnassigned = drafts.some((d) => d.project_id === null);
  if (!hasUnassigned || dismissed === null || dismissed === true) return null;

  return (
    <div role="alert" className="notification is-warning is-light mx-3 my-2 py-2 px-3">
      <p className="is-size-7">
        Some drafts are not linked to a workspace. Re-run /draft-post to link them.
      </p>
      <button className="button is-small is-warning mt-2" onClick={handleDismiss}>
        Dismiss
      </button>
    </div>
  );
}
