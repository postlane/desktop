// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4 -- Step 4: scheduler provider (Zernio only, matching
// ModalScheduler.tsx's existing VALID_PROVIDERS precedent -- see the
// checklist's scheduler-list scope decision), API key, optional profile ID.
//
// The API key stays in this step's local draft state and is only lifted
// into the wizard's aggregate config on submit -- it is never sent
// anywhere (no IPC call) until Step 6's final "Set up workspace" submit.

import { useState } from 'react';
import { Button, Group, NativeSelect, PasswordInput, Stack, Text, TextInput } from '@mantine/core';

export interface SchedulerPatch {
  scheduler_provider: string;
  scheduler_api_key: string;
  scheduler_profile_id: string | null;
}

interface Props {
  onNext: (patch: SchedulerPatch) => void;
  onBack: () => void;
}

export default function StepScheduler({ onNext, onBack }: Props) {
  const [apiKey, setApiKey] = useState('');
  const [profileId, setProfileId] = useState('');
  const [error, setError] = useState<string | null>(null);

  function handleSubmit() {
    if (!apiKey.trim()) {
      setError('API key is required.');
      return;
    }
    setError(null);
    onNext({
      scheduler_provider: 'zernio',
      scheduler_api_key: apiKey,
      scheduler_profile_id: profileId.trim() ? profileId.trim() : null,
    });
  }

  return (
    <Stack gap="sm">
      <NativeSelect label="Scheduler" data={[{ value: 'zernio', label: 'Zernio' }]} value="zernio" disabled />
      <PasswordInput label="API key" value={apiKey} onChange={(e) => setApiKey(e.currentTarget.value)} />
      <TextInput
        label="Profile ID"
        placeholder="Optional"
        value={profileId}
        onChange={(e) => setProfileId(e.currentTarget.value)}
      />
      {error && <Text size="sm" c="red">{error}</Text>}
      <Group justify="space-between">
        <Button variant="subtle" onClick={onBack}>Back</Button>
        <Button onClick={handleSubmit}>Next</Button>
      </Group>
    </Stack>
  );
}
