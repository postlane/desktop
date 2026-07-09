// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4 -- Step 3: LLM provider, model (prefilled per provider,
// free-text for providers with no curated list), data-disclosure notice
// (exact copy per postlane_build_brief_v2.0.md, shared with the CLI --
// not rewritten here).

import { useState } from 'react';
import { Button, Group, NativeSelect, Stack, Text, TextInput } from '@mantine/core';
import { LLM_MODEL_CHOICES, LLM_PROVIDER_LABELS, LLM_PROVIDERS } from '../../constants/llmProviders';

export interface LlmPatch {
  llm_provider: string;
  llm_model: string;
}

interface Props {
  onNext: (patch: LlmPatch) => void;
  onBack: () => void;
}

const PROVIDER_OPTIONS = LLM_PROVIDERS.map((p) => ({ value: p, label: LLM_PROVIDER_LABELS[p] ?? p }));

function defaultModelFor(provider: string): string {
  return LLM_MODEL_CHOICES[provider]?.[0] ?? '';
}

export default function StepLlm({ onNext, onBack }: Props) {
  const [provider, setProvider] = useState('anthropic');
  const [model, setModel] = useState(defaultModelFor('anthropic'));
  const [error, setError] = useState<string | null>(null);

  function handleProviderChange(value: string) {
    setProvider(value);
    setModel(defaultModelFor(value));
    setError(null);
  }

  function handleSubmit() {
    if (!model.trim()) {
      setError('Model is required.');
      return;
    }
    setError(null);
    onNext({ llm_provider: provider, llm_model: model.trim() });
  }

  const curatedModels = LLM_MODEL_CHOICES[provider];
  const providerLabel = LLM_PROVIDER_LABELS[provider] ?? provider;

  return (
    <Stack gap="sm">
      <NativeSelect
        label="Provider"
        data={PROVIDER_OPTIONS}
        value={provider}
        onChange={(e) => handleProviderChange(e.currentTarget.value)}
      />
      {curatedModels ? (
        <NativeSelect label="Model" data={curatedModels} value={model} onChange={(e) => setModel(e.currentTarget.value)} />
      ) : (
        <TextInput label="Model" value={model} onChange={(e) => setModel(e.currentTarget.value)} />
      )}
      <Text size="xs" c="dimmed">
        Post drafts and recent Git context will be sent to {providerLabel}…
      </Text>
      {error && <Text size="sm" c="red">{error}</Text>}
      <Group justify="space-between">
        <Button variant="subtle" onClick={onBack}>Back</Button>
        <Button onClick={handleSubmit}>Next</Button>
      </Group>
    </Stack>
  );
}
