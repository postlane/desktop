// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import WizardModal from './WizardModal'
import { Input } from '../components/catalyst/input'
import { Field, Label } from '../components/catalyst/fieldset'
import { Text } from '../components/catalyst/text'

interface Props {
  onNext: (projectId: string) => void
  onBack: () => void
}

interface CreateProjectResult {
  project_id: string
  name: string
  workspace_type: string
}

type WorkspaceType = 'personal' | 'organization' | 'client'

const WORKSPACE_TYPES: WorkspaceType[] = ['personal', 'organization', 'client']

function toWorkspaceType(value: string): WorkspaceType {
  const found = WORKSPACE_TYPES.find((t) => t === value)
  return found ?? 'personal'
}

function errorMessage(msg: string): string {
  if (msg.includes('no_free_slot')) {
    return 'You have no free workspace slot. Upgrade to add more workspaces.'
  }
  return `Failed to create workspace: ${msg}`
}

export default function ModalNameWorkspace({ onNext, onBack }: Props) {
  const [name, setName] = useState('')
  const [workspaceType, setWorkspaceType] = useState<WorkspaceType>('personal')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleNext() {
    setError(null)
    setLoading(true)
    try {
      const result = await invoke<CreateProjectResult>('create_project', { name, workspaceType })
      onNext(result.project_id)
    } catch (err) {
      setError(errorMessage(err instanceof Error ? err.message : String(err)))
    } finally {
      setLoading(false)
    }
  }

  return (
    <WizardModal
      title="Name your workspace"
      subtitle="A workspace is a brand, org, or client — it holds your scheduler and voice settings."
      onNext={handleNext}
      onBack={onBack}
      nextDisabled={name.trim().length === 0 || loading}
    >
      <div className="flex flex-col gap-4">
        {error && (
          <div role="alert" className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
            {error}
          </div>
        )}
        <Field>
          <Label>Workspace name</Label>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Postlane, Acme Corp, Personal"
          />
        </Field>
        <Field>
          <Label>Workspace type</Label>
          <select
            value={workspaceType}
            onChange={(e) => setWorkspaceType(toWorkspaceType(e.target.value))}
            className="block w-full rounded-md border border-zinc-300 bg-white px-3 py-2 text-sm text-zinc-900 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100"
          >
            <option value="personal">Personal</option>
            <option value="organization">Organization</option>
            <option value="client">Client project</option>
          </select>
        </Field>
        <Text className="text-sm">Your workspace name is visible only to you.</Text>
      </div>
    </WizardModal>
  )
}
