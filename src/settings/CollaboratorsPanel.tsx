// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.14/24.4.14a — collaborator list with promote/demote/remove,
// client-side search, and pagination past 25 rows.

import { useState, useMemo } from 'react';
import { TextInput, Pagination, Badge, Button, Group, Text, Stack } from '@mantine/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import { useCollaborators, type Collaborator } from '../hooks/useCollaborators';

const PAGE_SIZE = 25;

function CollaboratorRow({
  collaborator,
  onPromote,
  onDemote,
  onRemove,
}: {
  collaborator: Collaborator;
  onPromote: () => void;
  onDemote: () => void;
  onRemove: () => void;
}) {
  const label = collaborator.display_name ?? collaborator.user_id;
  return (
    <Group justify="space-between" py={4}>
      <Group gap="xs">
        <Text size="sm">{label}</Text>
        <Badge size="sm" color={collaborator.role === 'admin' ? 'blue' : 'gray'}>{collaborator.role}</Badge>
      </Group>
      <Group gap="xs">
        {collaborator.role === 'admin'
          ? <Button size="xs" variant="subtle" onClick={onDemote}>Demote to member</Button>
          : <Button size="xs" variant="subtle" onClick={onPromote}>Promote to admin</Button>}
        <Button size="xs" variant="subtle" color="red" onClick={onRemove}>Remove</Button>
      </Group>
    </Group>
  );
}

export default function CollaboratorsPanel({ projectId }: { projectId: string }) {
  const { collaborators, loading, error, actionError, setRole, remove } = useCollaborators(projectId);
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);

  const filtered = useMemo(
    () => collaborators.filter((c) => (c.display_name ?? c.user_id).toLowerCase().includes(search.toLowerCase())),
    [collaborators, search],
  );
  const showPagingControls = collaborators.length > PAGE_SIZE;
  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const pageItems = showPagingControls ? filtered.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE) : filtered;

  async function handleRemove(userId: string, label: string) {
    const yes = await confirm(
      `Remove ${label} from this workspace? They will immediately lose access to its drafts, queue, and history.`,
      { title: 'Remove collaborator', kind: 'warning' },
    );
    if (!yes) return;
    remove(userId);
  }

  if (loading) return <Text size="sm" c="dimmed">Loading collaborators…</Text>;
  if (error) return <Text size="sm" c="red">{error}</Text>;
  if (collaborators.length === 0) return <Text size="sm" c="dimmed">No collaborators on this workspace.</Text>;

  return (
    <Stack gap="xs">
      {showPagingControls && (
        <TextInput
          placeholder="Search collaborators…"
          value={search}
          onChange={(e) => { setSearch(e.currentTarget.value); setPage(1); }}
          size="xs"
        />
      )}
      {actionError && <Text size="sm" c="red">{actionError}</Text>}
      {pageItems.map((c) => (
        <CollaboratorRow
          key={c.user_id}
          collaborator={c}
          onPromote={() => setRole(c.user_id, 'admin')}
          onDemote={() => setRole(c.user_id, 'member')}
          onRemove={() => handleRemove(c.user_id, c.display_name ?? c.user_id)}
        />
      ))}
      {showPagingControls && totalPages > 1 && <Pagination total={totalPages} value={page} onChange={setPage} size="xs" />}
    </Stack>
  );
}
