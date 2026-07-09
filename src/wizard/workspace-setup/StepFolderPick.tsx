// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4/24.3.5 -- Step 1: pick a workspace folder, discover its
// child git repos, and show the assigned posts_dir for each before advancing.

import { useState } from 'react';
import { Button, Text, Stack, Table } from '@mantine/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '../../ipc/invoke';
import type { ChildRepo } from './types';

interface Props {
  onNext: (workspacePath: string, childRepos: ChildRepo[]) => void;
}

export default function StepFolderPick({ onNext }: Props) {
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [discovered, setDiscovered] = useState<{ path: string; repos: ChildRepo[] } | null>(null);

  async function handleChooseFolder() {
    const selected = await openDialog({ directory: true });
    if (!selected || typeof selected !== 'string') return;

    setError(null);
    setLoading(true);
    try {
      const repos = await invoke<ChildRepo[]>('discover_child_repos', { path: selected });
      setDiscovered({ path: selected, repos });
      onNext(selected, repos);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setDiscovered(null);
    } finally {
      setLoading(false);
    }
  }

  return (
    <Stack gap="sm">
      <Text size="sm">Pick the folder that contains your Git repositories.</Text>
      <Button onClick={handleChooseFolder} loading={loading} style={{ alignSelf: 'flex-start' }}>
        Choose folder
      </Button>
      {error && <Text size="sm" c="red">{error}</Text>}
      {discovered && (
        <Table>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Repository</Table.Th>
              <Table.Th>Posts folder</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {discovered.repos.map((repo) => (
              <Table.Tr key={repo.path}>
                <Table.Td>{repo.name}</Table.Td>
                <Table.Td>{repo.posts_dir}</Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      )}
    </Stack>
  );
}
