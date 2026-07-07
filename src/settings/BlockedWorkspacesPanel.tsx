// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.15a — account-deletion 409 resolution UI: per blocked
// workspace, either "Transfer to..." an eligible admin collaborator or
// "Start 14-day departure window"; zero eligible admins shows a
// "Promote a collaborator to admin first" fallback instead of a dead-end
// Transfer button. Deletion is re-attempted automatically once every
// listed workspace is resolved.

import { useState } from 'react';
import { NativeSelect, Button, Stack, Text, Group, Paper } from '@mantine/core';
import { invoke } from '../ipc/invoke';

export interface AdminCollaboratorInfo { user_id: string; display_name: string | null; }
export interface BlockedWorkspace { project_id: string; admin_collaborators: AdminCollaboratorInfo[]; }

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function WorkspaceRow({
  workspace,
  onResolved,
  onPromoteFirst,
}: {
  workspace: BlockedWorkspace;
  onResolved: (projectId: string) => void;
  onPromoteFirst: () => void;
}) {
  const [target, setTarget] = useState<string>('');
  const [actionError, setActionError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const hasEligibleAdmins = workspace.admin_collaborators.length > 0;

  async function runAction(command: string, args: Record<string, string>) {
    setBusy(true);
    setActionError(null);
    try {
      await invoke(command, args);
      onResolved(workspace.project_id);
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const handleTransfer = () => {
    if (!target) return;
    runAction('transfer_workspace_to_admin', { projectId: workspace.project_id, targetUserId: target });
  };

  const handleInitiateDeparture = () => runAction('initiate_ownership_departure', { projectId: workspace.project_id });

  return (
    <Paper withBorder p="sm">
      <Stack gap="xs">
        <Text size="sm" fw={600}>Workspace {workspace.project_id}</Text>
        {actionError && <Text size="sm" c="red">{actionError}</Text>}
        {hasEligibleAdmins ? (
          <Group gap="xs" wrap="nowrap">
            <NativeSelect
              aria-label="Choose an admin collaborator"
              data={[
                { value: '', label: 'Choose an admin collaborator' },
                ...workspace.admin_collaborators.map((c) => ({ value: c.user_id, label: c.display_name ?? c.user_id })),
              ]}
              value={target}
              onChange={(e) => setTarget(e.currentTarget.value)}
              size="xs"
              style={{ flex: 1 }}
            />
            <Button size="xs" disabled={!target || busy} onClick={handleTransfer}>Transfer to…</Button>
          </Group>
        ) : (
          <Button size="xs" variant="subtle" onClick={onPromoteFirst}>Promote a collaborator to admin first</Button>
        )}
        <Button size="xs" variant="light" color="orange" disabled={busy} onClick={handleInitiateDeparture}>
          Start 14-day departure window
        </Button>
      </Stack>
    </Paper>
  );
}

export default function BlockedWorkspacesPanel({
  workspaces,
  onAllResolved,
  onPromoteFirst,
}: {
  workspaces: BlockedWorkspace[];
  onAllResolved: () => void;
  onPromoteFirst: () => void;
}) {
  const [pending, setPending] = useState(workspaces);

  function handleResolved(projectId: string) {
    setPending((prev) => {
      const next = prev.filter((w) => w.project_id !== projectId);
      if (next.length === 0) onAllResolved();
      return next;
    });
  }

  return (
    <Stack gap="sm">
      <Text size="sm">
        This account owns {pending.length === 1 ? 'a workspace' : `${pending.length} workspaces`} with active
        collaborators. Resolve each one below to continue deleting your account.
      </Text>
      {pending.map((workspace) => (
        <WorkspaceRow
          key={workspace.project_id}
          workspace={workspace}
          onResolved={handleResolved}
          onPromoteFirst={onPromoteFirst}
        />
      ))}
    </Stack>
  );
}
