// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }))

import { invoke } from '../ipc/invoke'
import VoiceGuideBlock from './VoiceGuideBlock'

const mockInvoke = vi.mocked(invoke)

beforeEach(() => {
  vi.clearAllMocks()
  mockInvoke.mockImplementation(async (cmd) => {
    if (cmd === 'get_voice_guide_fields') return null
    if (cmd === 'save_project_voice_guide') return { synced: [], registered: 0 }
    return null
  })
})

// ── Load ───────────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — load', () => {
  it('calls get_voice_guide_fields on mount', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_voice_guide_fields', { projectId: 'proj-1' })
    )
  })

  it('owner sees Identity input', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => expect(screen.getByLabelText(/Identity/i)).toBeInTheDocument())
  })

  it('pre-populates Identity from stored fields', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return { description: 'Dev blogger', audience: '', tone: '', avoid: '', examples: '' }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByLabelText(/Identity/i)).toHaveValue('Dev blogger')
    )
  })

  it('pre-populates Tone from stored fields', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return { description: '', audience: '', tone: 'Casual', avoid: '', examples: '' }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => expect(screen.getByLabelText(/Tone/i)).toHaveValue('Casual'))
  })

  it('shows all five form fields for owner', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    expect(screen.getByLabelText(/Audience/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/Tone/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/Avoid/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/Example posts/i)).toBeInTheDocument()
  })
})

// ── Load error ─────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — load error', () => {
  it('shows error message when get_voice_guide_fields rejects', async () => {
    mockInvoke.mockRejectedValue(new Error('network'))
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/Failed to load voice guide/i)).toBeInTheDocument())
  })

  it('shows Retry button in error state', async () => {
    mockInvoke.mockRejectedValue(new Error('network'))
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument())
  })

  it('Retry re-invokes get_voice_guide_fields', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('network'))
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return null
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Retry/i }))
    fireEvent.click(screen.getByRole('button', { name: /Retry/i }))
    await waitFor(() => expect(screen.queryByText(/Failed to load voice guide/i)).not.toBeInTheDocument())
  })
})

// ── Templates ──────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — templates', () => {
  it('shows Professional template button', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Professional/i })).toBeInTheDocument()
    )
  })

  it('shows Conversational template button', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Conversational/i })).toBeInTheDocument()
    )
  })

  it('shows Technical template button', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /Technical/i })).toBeInTheDocument()
    )
  })

  it('clicking Professional fills the Audience field', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Professional/i }))
    fireEvent.click(screen.getByRole('button', { name: /Professional/i }))
    expect(screen.getByLabelText(/Audience/i)).not.toHaveValue('')
  })

  it('clicking Technical fills the Tone field', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Technical/i }))
    fireEvent.click(screen.getByRole('button', { name: /Technical/i }))
    expect(screen.getByLabelText(/Tone/i)).not.toHaveValue('')
  })

  it('template buttons are hidden for non-owners', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={false} />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled())
    expect(screen.queryByRole('button', { name: /Professional/i })).not.toBeInTheDocument()
  })
})

// ── Save ───────────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — save', () => {
  it('Save button is disabled when fields match loaded state', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    expect(screen.getByRole('button', { name: /^Save$/i })).toBeDisabled()
  })

  it('Save button is enabled after editing a field', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    expect(screen.getByRole('button', { name: /^Save$/i })).not.toBeDisabled()
  })

  it('calls save_project_voice_guide with voiceGuideFields', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', expect.objectContaining({
        projectId: 'proj-1',
        voiceGuideFields: expect.objectContaining({ description: 'My org' }),
      }))
    )
  })

  it('calls save_project_voice_guide with voiceGuide text', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find((c) => c[0] === 'save_project_voice_guide')
      expect(typeof (call?.[1] as Record<string, unknown>)?.voiceGuide).toBe('string')
    })
  })

  it('shows success message after save', async () => {
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() => expect(screen.getByText(/Voice guide saved/i)).toBeInTheDocument())
  })
})

// ── Sync confirmation (21.3.7 / 21.3.8) ───────────────────────────────────────

describe('VoiceGuideBlock — sync confirmation', () => {
  it('shows "synced to N repo(s)" when at least one repo was written (21.3.7)', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return null
      if (cmd === 'save_project_voice_guide') return { synced: ['/repos/my-repo'], registered: 1 }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() =>
      expect(screen.getByText(/synced to 1 repo/i)).toBeInTheDocument()
    )
  })

  it('shows "Connect a repository" when no repos are registered (21.3.8a)', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return null
      if (cmd === 'save_project_voice_guide') return { synced: [], registered: 0 }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() =>
      expect(screen.getByText(/Connect a repository to sync it there/i)).toBeInTheDocument()
    )
  })

  it('shows disk-path warning when repos are registered but all paths missing (21.3.8b)', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return null
      if (cmd === 'save_project_voice_guide') return { synced: [], registered: 2 }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={true} />)
    await waitFor(() => screen.getByLabelText(/Identity/i))
    fireEvent.change(screen.getByLabelText(/Identity/i), { target: { value: 'My org' } })
    fireEvent.click(screen.getByRole('button', { name: /^Save$/i }))
    await waitFor(() =>
      expect(screen.getByText(/repo paths could not be found on disk/i)).toBeInTheDocument()
    )
  })
})

// ── Non-owner ──────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — non-owner', () => {
  it('non-owner sees rendered text not editable fields', async () => {
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_voice_guide_fields') return { description: 'Dev', audience: 'Founders', tone: 'Direct', avoid: '', examples: '' }
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" projectName="Postlane" isOwner={false} />)
    await waitFor(() => expect(screen.queryByLabelText(/Identity/i)).not.toBeInTheDocument())
    expect(screen.queryByRole('button', { name: /^Save$/i })).not.toBeInTheDocument()
  })
})

// ── AI-M6: template content must not contain forbidden phrases ─────────────────

import { VOICE_GUIDE_TEMPLATES, FORBIDDEN_PHRASES_PATTERNS } from './VoiceGuideForm'

describe('VoiceGuideBlock — templates must not use forbidden phrases (AI-M6)', () => {
  for (const template of VOICE_GUIDE_TEMPLATES) {
    it(`template "${template.label}" has no forbidden phrases in non-avoid fields`, () => {
      const checkable = [template.fields.description, template.fields.audience, template.fields.tone, template.fields.examples]
      for (const field of checkable) {
        const lower = field.toLowerCase()
        for (const phrase of FORBIDDEN_PHRASES_PATTERNS) {
          expect(lower, `"${template.label}" contains "${phrase}"`).not.toContain(phrase)
        }
      }
    })
  }
})
