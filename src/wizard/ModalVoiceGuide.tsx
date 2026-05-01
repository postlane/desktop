// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Textarea } from '../components/catalyst/textarea'
import { Field, Label } from '../components/catalyst/fieldset'
import { Text } from '../components/catalyst/text'

interface Props {
  projectId: string
  onNext: (voiceGuide: string) => void
  onBack: () => void
  onSkip: () => void
}

export default function ModalVoiceGuide({ projectId, onNext, onBack, onSkip }: Props) {
  const [text, setText] = useState('')
  const [error, setError] = useState<string | null>(null)

  async function handleNext() {
    try {
      await invoke('save_project_voice_guide', { projectId, voiceGuide: text })
      onNext(text)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save voice guide.')
    }
  }

  async function handleSkip() {
    try {
      await invoke('save_project_voice_guide', { projectId, voiceGuide: '' })
      onSkip()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save voice guide.')
    }
  }

  return (
    <WizardModal
      title="Your writing voice"
      subtitle="This guide tells Postlane how you write. It applies to every repo in this project."
      onNext={handleNext}
      onBack={onBack}
      onSkip={handleSkip}
    >
      <Field>
        <Label>Voice guide</Label>
        <Textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          maxLength={5000}
          rows={6}
          placeholder="E.g. Direct and technical. No hype. Write as if explaining to a smart colleague."
        />
        <Text className="mt-1 text-xs">{text.length}/5000</Text>
      </Field>
      {error !== null && (
        <p role="alert" className="mt-2 text-sm text-red-600">{error}</p>
      )}
    </WizardModal>
  )
}
