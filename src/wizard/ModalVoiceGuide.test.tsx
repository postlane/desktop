// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))
import { invoke } from '@tauri-apps/api/core'

import ModalVoiceGuide from './ModalVoiceGuide'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => { vi.clearAllMocks() })

describe('ModalVoiceGuide', () => {
  it('test_skip_saves_empty', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const onSkip = vi.fn()
    render(<ModalVoiceGuide projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} onSkip={onSkip} />)
    fireEvent.click(screen.getByRole('button', { name: /skip/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', { projectId: 'proj-1', voiceGuide: '' })
      expect(onSkip).toHaveBeenCalledOnce()
    })
  })

  it('test_next_saves_content', async () => {
    mockInvoke.mockResolvedValue(undefined)
    const onNext = vi.fn()
    render(<ModalVoiceGuide projectId="proj-1" onNext={onNext} onBack={vi.fn()} onSkip={vi.fn()} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Direct and technical.' } })
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', { projectId: 'proj-1', voiceGuide: 'Direct and technical.' })
      expect(onNext).toHaveBeenCalledWith('Direct and technical.')
    })
  })

  it('test_shows_error_when_invoke_throws_on_next', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC failed'))
    render(<ModalVoiceGuide projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} onSkip={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /next/i }))
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
    })
  })

  it('test_shows_error_when_invoke_throws_on_skip', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC failed'))
    render(<ModalVoiceGuide projectId="proj-1" onNext={vi.fn()} onBack={vi.fn()} onSkip={vi.fn()} />)
    fireEvent.click(screen.getByRole('button', { name: /skip/i }))
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
    })
  })
})
