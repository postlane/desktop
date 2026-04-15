// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Button } from '../components/catalyst/button';
import {
  Dialog,
  DialogActions,
  DialogBody,
  DialogDescription,
  DialogTitle,
} from '../components/catalyst/dialog';

interface Props {
  onClose: () => void;
}

export default function AddRepoModal({ onClose }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleBrowse() {
    setError(null);
    const selected = await openDialog({ directory: true });
    if (!selected) return;

    setLoading(true);
    try {
      await invoke('add_repo', { path: selected });
      onClose();
    } catch (e) {
      setError("This folder hasn't been set up yet. Run `npx postlane init` inside it first.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <Dialog open onClose={onClose}>
      <DialogTitle>Add a repo</DialogTitle>
      <DialogDescription>
        Select a folder where you've already run{' '}
        <code className="font-mono text-sm">npx postlane init</code>.
      </DialogDescription>
      <DialogBody>
        {error && (
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        )}
      </DialogBody>
      <DialogActions>
        <Button outline onClick={onClose}>Cancel</Button>
        <Button onClick={handleBrowse} disabled={loading}>
          {loading ? 'Adding…' : 'Browse for the folder'}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
