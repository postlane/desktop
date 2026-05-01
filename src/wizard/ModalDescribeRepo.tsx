// SPDX-License-Identifier: BUSL-1.1

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Input } from '../components/catalyst/input'
import { Field, Label } from '../components/catalyst/fieldset'

interface Props {
  projectId: string
  repoPath: string
  onNext: (description: string) => void
  onBack: () => void
}

export default function ModalDescribeRepo({ projectId, repoPath, onNext, onBack }: Props) {
  const [description, setDescription] = useState('')

  useEffect(() => {
    invoke<string | null>('get_repo_remote_name', { repoPath })
      .then((name) => { if (name) setDescription(name) })
      .catch(() => { /* ignore */ })
  }, [repoPath])

  async function handleNext() {
    await invoke('register_repo_with_project', { projectId, repoPath, description })
    onNext(description)
  }

  return (
    <WizardModal
      title="What is this repo?"
      subtitle="This helps Postlane draft posts that are specific to this codebase."
      onNext={handleNext}
      onBack={onBack}
      nextDisabled={description.trim().length === 0}
    >
      <Field>
        <Label>Repo description</Label>
        <Input
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="e.g. A CLI tool for managing serverless deployments"
        />
      </Field>
    </WizardModal>
  )
}
