// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

export type WizardModalName =
  | 'welcome' | 'sign-in' | 'name-workspace' | 'connect-scheduler'
  | 'backup-scheduler' | 'map-profiles' | 'voice-guide' | 'connect-repo'
  | 'describe-repo' | 'platform-overrides' | 'done' | 'pricing-gate'

export interface WizardState {
  modal: WizardModalName
  projectId: string | null
  selectedRepoPath: string | null
  repoDescription: string
  voiceGuide: string
  schedulerConnected: boolean
  licenseTokenPresent: boolean
}

const INITIAL: WizardState = {
  modal: 'welcome',
  projectId: null,
  selectedRepoPath: null,
  repoDescription: '',
  voiceGuide: '',
  schedulerConnected: false,
  licenseTokenPresent: false,
}

const NEXT_MAP: Partial<Record<WizardModalName, WizardModalName>> = {
  'welcome': 'sign-in',
  'sign-in': 'name-workspace',
  'name-workspace': 'connect-scheduler',
  'connect-scheduler': 'backup-scheduler',
  'backup-scheduler': 'map-profiles',
  'map-profiles': 'voice-guide',
  'voice-guide': 'connect-repo',
  'connect-repo': 'describe-repo',
  'describe-repo': 'platform-overrides',
  'platform-overrides': 'done',
}

const BACK_MAP: Partial<Record<WizardModalName, WizardModalName>> = {
  'sign-in': 'welcome',
  'name-workspace': 'sign-in',
  'connect-scheduler': 'name-workspace',
  'backup-scheduler': 'connect-scheduler',
  'map-profiles': 'backup-scheduler',
  'voice-guide': 'map-profiles',
  'connect-repo': 'voice-guide',
  'describe-repo': 'connect-repo',
  'platform-overrides': 'describe-repo',
}

const SKIP_MAP: Partial<Record<WizardModalName, WizardModalName>> = {
  'backup-scheduler': 'map-profiles',
  'map-profiles': 'voice-guide',
  'voice-guide': 'connect-repo',
  'platform-overrides': 'done',
}

function canAdvance(modal: WizardModalName, s: WizardState): boolean {
  if (modal === 'sign-in') return s.licenseTokenPresent
  if (modal === 'connect-scheduler') return s.schedulerConnected
  if (modal === 'name-workspace') return s.projectId !== null
  if (modal === 'connect-repo') return s.selectedRepoPath !== null
  if (modal === 'describe-repo') return s.repoDescription.trim().length > 0
  return true
}

function applyNext(s: WizardState): WizardState {
  if (!canAdvance(s.modal, s)) return s
  const to = NEXT_MAP[s.modal]
  return to ? { ...s, modal: to } : s
}

function applyBack(s: WizardState): WizardState {
  const to = BACK_MAP[s.modal]
  return to ? { ...s, modal: to } : s
}

function applySkip(s: WizardState): WizardState {
  const to = SKIP_MAP[s.modal]
  return to ? { ...s, modal: to } : s
}

async function resolveGate(): Promise<WizardModalName> {
  try {
    const gate = await invoke<string>('check_billing_gate')
    return gate === 'free' || gate === 'paid' ? 'name-workspace' : 'pricing-gate'
  } catch {
    return 'pricing-gate'
  }
}

export function useWizardState() {
  const [state, setState] = useState<WizardState>(INITIAL)

  const next = () => setState(applyNext)
  const back = () => setState(applyBack)
  const skip = () => setState(applySkip)

  const startFirstLaunch = () => setState({ ...INITIAL, modal: 'welcome' })
  const startAddRepo = (projectId: string) => setState({ ...INITIAL, modal: 'connect-repo', projectId })

  async function startAddProject() {
    const modal = await resolveGate()
    setState((s) => ({ ...s, modal }))
  }

  const setLicenseTokenPresent = (v: boolean) => setState((s) => ({ ...s, licenseTokenPresent: v }))
  const setSchedulerConnected = (v: boolean) => setState((s) => ({ ...s, schedulerConnected: v }))
  const setProjectId = (id: string) => setState((s) => ({ ...s, projectId: id }))
  const setSelectedRepoPath = (p: string) => setState((s) => ({ ...s, selectedRepoPath: p }))
  const setRepoDescription = (d: string) => setState((s) => ({ ...s, repoDescription: d }))
  const setVoiceGuide = (g: string) => setState((s) => ({ ...s, voiceGuide: g }))
  const advanceScheduler = () => setState((s) => ({ ...s, modal: 'backup-scheduler' }))
  const advancePastGate = () => setState((s) => ({ ...s, modal: 'name-workspace' }))
  const jumpToDone = () => setState((s) => ({ ...s, modal: 'done' }))
  const jumpToPricingGate = () => setState((s) => ({ ...s, modal: 'pricing-gate' }))

  return {
    state, next, back, skip,
    startFirstLaunch, startAddProject, startAddRepo,
    setLicenseTokenPresent, setSchedulerConnected,
    setProjectId, setSelectedRepoPath, setRepoDescription, setVoiceGuide,
    advanceScheduler, advancePastGate, jumpToDone, jumpToPricingGate,
  }
}
