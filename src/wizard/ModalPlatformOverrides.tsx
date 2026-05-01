// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Switch, SwitchField, SwitchGroup } from '../components/catalyst/switch'
import { Label } from '../components/catalyst/fieldset'

interface Props {
  repoPath: string
  onNext: () => void
  onBack: () => void
  onSkip: () => void
}

const PLATFORMS = [
  { id: 'x', label: 'X (Twitter)' },
  { id: 'bluesky', label: 'Bluesky' },
  { id: 'linkedin', label: 'LinkedIn' },
  { id: 'mastodon', label: 'Mastodon' },
] as const

type PlatformId = typeof PLATFORMS[number]['id']

type Overrides = Record<PlatformId, boolean>

const DEFAULTS: Overrides = { x: true, bluesky: true, linkedin: true, mastodon: true }

export default function ModalPlatformOverrides({ repoPath, onNext, onBack, onSkip }: Props) {
  const [overrides, setOverrides] = useState<Overrides>(DEFAULTS)

  function toggle(platform: PlatformId) {
    setOverrides((prev) => ({ ...prev, [platform]: !prev[platform] }))
  }

  async function saveAndAdvance(cb: () => void) {
    await invoke('save_repo_platform_overrides', { repoPath, overrides })
    cb()
  }

  return (
    <WizardModal
      title="Platform overrides"
      subtitle="By default, this repo posts to all platforms configured for this project. Turn any off here."
      onNext={() => saveAndAdvance(onNext)}
      onBack={onBack}
      onSkip={() => saveAndAdvance(onSkip)}
    >
      <SwitchGroup>
        {PLATFORMS.map((p) => (
          <SwitchField key={p.id}>
            <Label>{p.label}</Label>
            <Switch
              checked={overrides[p.id]}
              onChange={() => toggle(p.id)}
              aria-label={p.label}
            />
          </SwitchField>
        ))}
      </SwitchGroup>
    </WizardModal>
  )
}
