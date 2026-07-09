// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.6/24.3.6a -- Step 6: review + confirm, then "Set up
// workspace" wires to setup_workspace, followed by a separate
// get_workspace_billing_status round-trip so a billing-status failure
// never masks an otherwise-successful workspace creation (and can be
// retried on its own). workspace_upgrade_prompted telemetry fires
// server-side inside that command, not from this component.

import { useState } from 'react';
import { Alert, Button, Group, List, Stack, Text } from '@mantine/core';
import { invoke } from '../../ipc/invoke';
import type { ChildRepo, WorkspaceConfig } from './types';

const SUCCESS_MESSAGE = 'Workspace connected. Invoke /draft-post in your IDE to create your first post.';

interface Props {
  workspacePath: string;
  childRepos: ChildRepo[];
  config: WorkspaceConfig;
  onBack: () => void;
  onComplete: () => void;
  onUpgradeClick: () => void;
}

type Status = 'idle' | 'submitting' | 'success';

function ConfigSummary({ config, childRepos }: { config: WorkspaceConfig; childRepos: ChildRepo[] }) {
  return (
    <Stack gap="xs">
      <Text size="sm"><strong>Author:</strong> {config.author}</Text>
      <Text size="sm"><strong>Style:</strong> {config.style}</Text>
      <Text size="sm"><strong>Platforms:</strong> {config.platforms.join(', ')}</Text>
      <Text size="sm"><strong>LLM:</strong> {config.llm_provider} / {config.llm_model}</Text>
      <Text size="sm" fw={600}>Repositories</Text>
      <List size="sm">
        {childRepos.map((repo) => <List.Item key={repo.path}>{repo.name}</List.Item>)}
      </List>
    </Stack>
  );
}

async function fetchBillingStatus(projectId: string): Promise<string | null> {
  try {
    const result = await invoke<{ status: string }>('get_workspace_billing_status', { projectId });
    return result.status;
  } catch (e) {
    console.error('[workspace-setup] billing-status check failed:', e);
    return null;
  }
}

export default function StepReview({ workspacePath, childRepos, config, onBack, onComplete, onUpgradeClick }: Props) {
  const [status, setStatus] = useState<Status>('idle');
  const [error, setError] = useState<string | null>(null);
  const [billingStatus, setBillingStatus] = useState<string | null>(null);

  async function handleSubmit() {
    if (status === 'submitting') return;
    setStatus('submitting');
    setError(null);
    try {
      await invoke('setup_workspace', { path: workspacePath, config, childRepos });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStatus('idle');
      return;
    }
    setBillingStatus(await fetchBillingStatus(config.project_id));
    setStatus('success');
  }

  async function handleContinue() {
    invoke('clear_wizard_state').catch(console.warn);
    try { await invoke('set_wizard_completed'); } catch (e) { console.warn('[wizard] set_wizard_completed failed:', e); }
    onComplete();
  }

  if (status === 'success') {
    return (
      <Stack gap="sm">
        <Text size="sm">{SUCCESS_MESSAGE}</Text>
        {billingStatus === 'paid_required' && (
          <Alert color="blue">
            This is your second workspace — it costs $5/month.{' '}
            <Button variant="subtle" size="xs" onClick={onUpgradeClick}>Add to plan</Button>
          </Alert>
        )}
        <Group justify="flex-end">
          <Button onClick={handleContinue}>Continue</Button>
        </Group>
      </Stack>
    );
  }

  return (
    <Stack gap="sm">
      <ConfigSummary config={config} childRepos={childRepos} />
      {error && <Text size="sm" c="red">{error}</Text>}
      <Group justify="space-between">
        <Button variant="subtle" onClick={onBack} disabled={status === 'submitting'}>Back</Button>
        <Button onClick={handleSubmit} loading={status === 'submitting'} disabled={status === 'submitting'}>
          Set up workspace
        </Button>
      </Group>
    </Stack>
  );
}
