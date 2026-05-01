// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import WizardModal from './WizardModal'
import { Button } from '../components/catalyst/button'
import { Text } from '../components/catalyst/text'

interface Props {
  projectId: string
  onNext: (repoPath: string) => void
  onBack: () => void
  onSilentAdd?: () => void
  onPricingGate?: () => void
}

type DetectionOutcome = 'silent' | 'gate' | 'picker' | 'normal'
type WorkspaceChoice = 'current' | 'new'

function classifyError(msg: string): string {
  if (msg.toLowerCase().includes('git')) return "This directory isn't a git repository."
  if (msg.toLowerCase().includes('config')) return 'Run `npx @postlane/cli init` in this directory first.'
  return `Error: ${msg}`
}

async function silentAdd(path: string): Promise<void> {
  await invoke('add_repo', { path })
}

async function handleNotFoundStatus(path: string): Promise<DetectionOutcome> {
  const gate = await invoke<string>('check_billing_gate')
  if (gate === 'free') { await silentAdd(path); return 'silent' }
  if (gate === 'none') return 'gate'
  return 'normal'
}

async function handleDetection(path: string, existingProjectId: string): Promise<DetectionOutcome> {
  const status = await invoke<string>('check_project_status', { projectId: existingProjectId })
  if (status === 'owned') { await silentAdd(path); return 'silent' }
  if (status === 'not_found') return handleNotFoundStatus(path)
  return 'normal'
}

interface WorkspacePickerProps {
  value: WorkspaceChoice
  onChange: (v: WorkspaceChoice) => void
  newName: string
  onNameChange: (n: string) => void
}

function WorkspacePicker({ value, onChange, newName, onNameChange }: WorkspacePickerProps) {
  const cls = 'rounded-lg border border-zinc-300 bg-white px-3 py-1.5 text-sm dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100'
  return (
    <div className="flex flex-col gap-2">
      <Text className="text-sm">This repo isn't linked to a workspace yet.</Text>
      <select aria-label="Select workspace" className={cls} value={value}
        onChange={(e) => onChange(e.target.value as WorkspaceChoice)}>
        <option value="current">Use current workspace</option>
        <option value="new">Create a new workspace</option>
      </select>
      {value === 'new' && (
        <input aria-label="Workspace name" type="text" value={newName} placeholder="My workspace"
          className={cls} onChange={(e) => onNameChange(e.target.value)} />
      )}
    </div>
  )
}

function usePickerState(projectId: string, onNext: (p: string) => void) {
  const [repoPath, setRepoPath] = useState<string | null>(null)
  const [commitNotice, setCommitNotice] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [showProjectPicker, setShowProjectPicker] = useState(false)
  const [workspaceChoice, setWorkspaceChoice] = useState<WorkspaceChoice>('current')
  const [newWorkspaceName, setNewWorkspaceName] = useState('')

  async function handlePickerNext() {
    if (!repoPath) return
    setLoading(true)
    try {
      let pid = projectId
      if (workspaceChoice === 'new') {
        const r = await invoke<{ project_id: string }>('create_project', { name: newWorkspaceName, workspaceType: 'personal' })
        pid = r.project_id
      }
      await invoke('add_repo', { path: repoPath })
      await invoke<string>('write_project_id_to_config', { repoPath, projectId: pid })
      setShowProjectPicker(false)
      onNext(repoPath)
    } catch (err) { setError(classifyError(err instanceof Error ? err.message : String(err))) }
    finally { setLoading(false) }
  }

  return {
    repoPath, setRepoPath, commitNotice, setCommitNotice, error, setError,
    loading, setLoading, showProjectPicker, setShowProjectPicker,
    workspaceChoice, setWorkspaceChoice, newWorkspaceName, setNewWorkspaceName,
    handlePickerNext,
  }
}

export default function ModalConnectRepo({ projectId, onNext, onBack, onSilentAdd, onPricingGate }: Props) {
  const s = usePickerState(projectId, onNext)
  const nextDisabled = s.repoPath === null || (s.showProjectPicker && s.workspaceChoice === 'new' && !s.newWorkspaceName.trim())

  async function handleBrowse() {
    const selected = await open({ directory: true })
    if (selected === null) return
    s.setError(null); s.setCommitNotice(null); s.setLoading(true)
    try {
      const existingId = await invoke<string | null>('read_project_id_from_path', { path: selected })
      if (existingId !== null) {
        const outcome = await handleDetection(selected, existingId)
        if (outcome === 'silent') { onSilentAdd?.(); return }
        if (outcome === 'gate') { onPricingGate?.(); return }
      } else { s.setShowProjectPicker(true); s.setRepoPath(selected); return }
      await invoke('add_repo', { path: selected })
      const notice = await invoke<string>('write_project_id_to_config', { repoPath: selected, projectId })
      s.setRepoPath(selected)
      s.setCommitNotice(notice)
    } catch (err) { s.setError(classifyError(err instanceof Error ? err.message : String(err))) }
    finally { s.setLoading(false) }
  }

  return (
    <WizardModal title="Connect a repo" subtitle="Postlane monitors this repo for approved post drafts."
      onNext={() => { if (s.repoPath) { s.showProjectPicker ? void s.handlePickerNext() : onNext(s.repoPath) } }}
      onBack={onBack} nextDisabled={nextDisabled}>
      <div className="flex flex-col gap-4">
        <Button outline onClick={handleBrowse} disabled={s.loading}>Browse</Button>
        {s.error && <Text className="text-sm text-red-600 dark:text-red-400">{s.error}</Text>}
        {s.commitNotice && <Text className="text-sm text-green-700 dark:text-green-400">{s.commitNotice}</Text>}
        {s.repoPath && !s.commitNotice && !s.showProjectPicker && <Text className="text-sm">{s.repoPath}</Text>}
        {s.showProjectPicker && <WorkspacePicker value={s.workspaceChoice} onChange={s.setWorkspaceChoice}
          newName={s.newWorkspaceName} onNameChange={s.setNewWorkspaceName} />}
      </div>
    </WizardModal>
  )
}
