// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.9 — workspace list with billing-status badges and actions.
// collaborator/inactive/unlicensed show a badge only; their actions
// (owner-name display, Reactivate, Take-over-billing) depend on backend
// support that doesn't exist yet (see design-brief-account-tab-update.md).

import { useState } from 'react';
import { Badge, Button, Group, Text, Stack } from '@mantine/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '../ipc/invoke';
import { useProjectsContext } from '../context/ProjectsProvider';
import type { Project, WorkspaceStatus } from '../types';

function statusBadge(status: WorkspaceStatus | undefined): { label: string; color: string } {
  switch (status) {
    case 'free_owned': return { label: 'Free', color: 'gray' };
    case 'paid_owned': return { label: '$5/month', color: 'green' };
    case 'paid_required': return { label: 'Unbilled', color: 'yellow' };
    case 'payment_failed': return { label: 'Payment failed', color: 'orange' };
    case 'collaborator': return { label: 'Collaborator', color: 'blue' };
    case 'inactive': return { label: 'Paused', color: 'gray' };
    case 'unlicensed': return { label: 'Unlicensed', color: 'red' };
    default: return { label: 'Unknown', color: 'gray' };
  }
}

type ActionKind = 'deactivate_workspace' | 'subscribe_workspace' | 'open_billing_portal';

function statusAction(status: WorkspaceStatus | undefined): { label: string; command: ActionKind } | null {
  switch (status) {
    case 'paid_owned': return { label: 'Pause', command: 'deactivate_workspace' };
    case 'paid_required': return { label: 'Add to plan', command: 'subscribe_workspace' };
    case 'payment_failed': return { label: 'Update billing', command: 'open_billing_portal' };
    default: return null;
  }
}

function errorMessage(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

function WorkspaceRow({ project, refresh }: { project: Project; refresh: () => void }) {
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const badge = statusBadge(project.status);
  const action = statusAction(project.status);

  async function runAction(command: ActionKind) {
    setBusy(true);
    setActionError(null);
    try {
      const result = await invoke<string | null | undefined>(command, { projectId: project.id });
      if (typeof result === 'string') await openUrl(result);
      if (command !== 'open_billing_portal') refresh();
    } catch (e) {
      setActionError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Stack gap={4}>
      <Group justify="space-between">
        <Text size="sm">{project.name}</Text>
        <Group gap="xs">
          <Badge size="sm" color={badge.color}>{badge.label}</Badge>
          {action && (
            <Button size="xs" loading={busy} onClick={() => runAction(action.command)}>
              {action.label}
            </Button>
          )}
        </Group>
      </Group>
      {actionError && <Text size="sm" c="red">{actionError}</Text>}
    </Stack>
  );
}

export default function WorkspaceListSection() {
  const { projects, refresh } = useProjectsContext();

  if (projects.length === 0) return null;

  return (
    <Stack gap="sm" className="mb-4">
      {projects.map((project) => (
        <WorkspaceRow key={project.id} project={project} refresh={refresh} />
      ))}
    </Stack>
  );
}
