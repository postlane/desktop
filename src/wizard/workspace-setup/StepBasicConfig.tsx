// SPDX-License-Identifier: BUSL-1.1
// checklist 24.3.4 -- Step 2: base URL, platforms checklist, conditional
// Mastodon instance field, author, writing style, UTM campaign.
//
// No "language" field here -- that's a separate item (24.9.2), not yet
// built, touching this same step later. Keep the submit patch as a plain
// object so a future field can be added without restructuring this step.

import { useState } from 'react';
import { Button, Checkbox, Group, Stack, Text, TextInput } from '@mantine/core';
import { PLATFORM_LABELS, PLATFORM_ORDER } from '../../constants/platforms';

const STEP_PLATFORMS = PLATFORM_ORDER.filter((p) => p !== 'substack');

export interface BasicConfigPatch {
  base_url: string;
  platforms: string[];
  mastodon_instance: string | null;
  author: string;
  style: string;
  utm_campaign: string | null;
}

interface Props {
  onNext: (patch: BasicConfigPatch) => void;
  onBack: () => void;
}

function PlatformChecklist({ platforms, onToggle }: { platforms: string[]; onToggle: (p: string) => void }) {
  return (
    <Stack gap="xs">
      <Text size="sm" fw={600}>Platforms</Text>
      {STEP_PLATFORMS.map((platform) => (
        <Checkbox
          key={platform}
          label={PLATFORM_LABELS[platform] ?? platform}
          checked={platforms.includes(platform)}
          onChange={() => onToggle(platform)}
        />
      ))}
    </Stack>
  );
}

interface ContentFieldsProps {
  showMastodonInstance: boolean;
  mastodonInstance: string;
  onMastodonInstanceChange: (v: string) => void;
  author: string;
  onAuthorChange: (v: string) => void;
  style: string;
  onStyleChange: (v: string) => void;
  utmCampaign: string;
  onUtmCampaignChange: (v: string) => void;
}

function ContentFields({
  showMastodonInstance, mastodonInstance, onMastodonInstanceChange,
  author, onAuthorChange, style, onStyleChange, utmCampaign, onUtmCampaignChange,
}: ContentFieldsProps) {
  return (
    <>
      {showMastodonInstance && (
        <TextInput
          label="Mastodon instance"
          placeholder="mastodon.social"
          value={mastodonInstance}
          onChange={(e) => onMastodonInstanceChange(e.currentTarget.value)}
        />
      )}
      <TextInput label="Author name" value={author} onChange={(e) => onAuthorChange(e.currentTarget.value)} />
      <TextInput
        label="Writing style"
        placeholder="e.g. Direct, no jargon, short sentences"
        value={style}
        onChange={(e) => onStyleChange(e.currentTarget.value)}
      />
      <TextInput
        label="UTM campaign"
        placeholder="Optional"
        value={utmCampaign}
        onChange={(e) => onUtmCampaignChange(e.currentTarget.value)}
      />
    </>
  );
}

export default function StepBasicConfig({ onNext, onBack }: Props) {
  const [baseUrl, setBaseUrl] = useState('https://postlane.dev');
  const [platforms, setPlatforms] = useState<string[]>([]);
  const [mastodonInstance, setMastodonInstance] = useState('');
  const [author, setAuthor] = useState('');
  const [style, setStyle] = useState('');
  const [utmCampaign, setUtmCampaign] = useState('');
  const [error, setError] = useState<string | null>(null);

  function togglePlatform(platform: string) {
    setPlatforms((prev) => (prev.includes(platform) ? prev.filter((p) => p !== platform) : [...prev, platform]));
  }

  function handleSubmit() {
    if (!baseUrl.startsWith('https://')) {
      setError('Base URL must start with https://');
      return;
    }
    if (platforms.length === 0) {
      setError('Select at least one platform.');
      return;
    }
    setError(null);
    onNext({
      base_url: baseUrl,
      platforms,
      mastodon_instance: platforms.includes('mastodon') && mastodonInstance.trim() ? mastodonInstance.trim() : null,
      author,
      style,
      utm_campaign: utmCampaign.trim() ? utmCampaign.trim() : null,
    });
  }

  return (
    <Stack gap="sm">
      <TextInput label="Base URL" value={baseUrl} onChange={(e) => setBaseUrl(e.currentTarget.value)} />
      <PlatformChecklist platforms={platforms} onToggle={togglePlatform} />
      <ContentFields
        showMastodonInstance={platforms.includes('mastodon')}
        mastodonInstance={mastodonInstance}
        onMastodonInstanceChange={setMastodonInstance}
        author={author}
        onAuthorChange={setAuthor}
        style={style}
        onStyleChange={setStyle}
        utmCampaign={utmCampaign}
        onUtmCampaignChange={setUtmCampaign}
      />
      {error && <Text size="sm" c="red">{error}</Text>}
      <Group justify="space-between">
        <Button variant="subtle" onClick={onBack}>Back</Button>
        <Button onClick={handleSubmit}>Next</Button>
      </Group>
    </Stack>
  );
}
