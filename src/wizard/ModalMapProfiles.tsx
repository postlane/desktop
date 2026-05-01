// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import WizardModal from './WizardModal'
import { Select } from '../components/catalyst/select'
import { Field, Label } from '../components/catalyst/fieldset'

interface Props {
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

type ProfileMap = Record<PlatformId, string>

const INITIAL_MAP: ProfileMap = { x: '', bluesky: '', linkedin: '', mastodon: '' }

export default function ModalMapProfiles({ onNext, onBack, onSkip }: Props) {
  const [profiles, setProfiles] = useState<ProfileMap>(INITIAL_MAP)

  function setProfile(platform: PlatformId, value: string) {
    setProfiles((prev) => ({ ...prev, [platform]: value }))
  }

  return (
    <WizardModal
      title="Link your social profiles"
      subtitle="Tell Postlane which account to post to on each platform."
      onNext={onNext}
      onBack={onBack}
      onSkip={onSkip}
    >
      <div className="flex flex-col gap-4">
        {PLATFORMS.map((p) => (
          <Field key={p.id}>
            <Label htmlFor={`profile-${p.id}`}>{p.label}</Label>
            <Select
              id={`profile-${p.id}`}
              value={profiles[p.id]}
              onChange={(e) => setProfile(p.id, e.target.value)}
            >
              <option value="">— Not configured —</option>
            </Select>
          </Field>
        ))}
      </div>
    </WizardModal>
  )
}
