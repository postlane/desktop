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
    if (cmd === 'get_project_voice_guide') return 'Be concise.'
    return null
  })
})

// ── Load ───────────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — load', () => {
  it('calls get_project_voice_guide on mount', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('get_project_voice_guide', { projectId: 'proj-1' }))
  })

  it('owner sees textarea with loaded content', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('textbox')).toHaveValue('Be concise.'))
  })

  it('non-owner sees pre with loaded content instead of textarea', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={false} />)
    await waitFor(() => {
      expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
      expect(screen.getByText('Be concise.')).toBeInTheDocument()
    })
  })
})

// ── Load error ─────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — load error', () => {
  it('shows error message when get_project_voice_guide rejects', async () => {
    mockInvoke.mockRejectedValue(new Error('network'))
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByText(/Failed to load voice guide/i)).toBeInTheDocument())
  })

  it('disables textarea while in error state', async () => {
    mockInvoke.mockRejectedValue(new Error('network'))
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('textbox')).toBeDisabled())
  })

  it('shows Retry button in error state', async () => {
    mockInvoke.mockRejectedValue(new Error('network'))
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => expect(screen.getByRole('button', { name: /Retry/i })).toBeInTheDocument())
  })

  it('Retry re-invokes get_project_voice_guide', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('network'))
    mockInvoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_project_voice_guide') return 'Be concise.'
      return null
    })
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('button', { name: /Retry/i }))
    fireEvent.click(screen.getByRole('button', { name: /Retry/i }))
    await waitFor(() => expect(screen.queryByText(/Failed to load voice guide/i)).not.toBeInTheDocument())
  })
})

// ── Save ───────────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — save', () => {
  it('Save button disabled when text equals loaded value', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    expect(screen.getByRole('button', { name: /Save/i })).toBeDisabled()
  })

  it('Save button enabled when text differs from loaded value', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'New content' } })
    expect(screen.getByRole('button', { name: /Save/i })).not.toBeDisabled()
  })

  it('calls save_project_voice_guide with correct args', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', {
      projectId: 'proj-1', voiceGuide: 'Updated',
    }))
  })

  it('shows success message after save', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } })
    fireEvent.click(screen.getByRole('button', { name: /Save/i }))
    await waitFor(() => expect(screen.getByText(/Voice guide saved/i)).toBeInTheDocument())
  })
})

// ── Over-limit ─────────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — over limit', () => {
  it('Save button disabled with tooltip when text > 5000 chars', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(5001) } })
    const btn = screen.getByRole('button', { name: /Save/i })
    expect(btn).toBeDisabled()
    expect(btn).toHaveAttribute('title', expect.stringContaining('5000'))
  })

  it('character count shows in danger color when over limit', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(5001) } })
    expect(screen.getByTestId('char-count')).toHaveClass('has-text-danger')
  })
})

// ── Template button ────────────────────────────────────────────────────────────

describe('VoiceGuideBlock — templates', () => {
  it('template button opens dropdown', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    expect(screen.getByText(/Professional & direct/i)).toBeInTheDocument()
  })

  it('selecting template applies immediately when no unsaved changes', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    fireEvent.click(screen.getByText(/Professional & direct/i))
    expect(screen.getByRole('textbox')).not.toHaveValue('Be concise.')
    expect(screen.queryByText(/Replace your current text/i)).not.toBeInTheDocument()
  })

  it('selecting template when unsaved changes shows confirmation', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'My custom guide' } })
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    fireEvent.click(screen.getByText(/Professional & direct/i))
    expect(screen.getByText(/Replace your current text/i)).toBeInTheDocument()
  })

  it('Cancel in template confirm keeps current text', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'My custom guide' } })
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    fireEvent.click(screen.getByText(/Professional & direct/i))
    fireEvent.click(screen.getByRole('button', { name: /^Cancel$/i }))
    expect(screen.getByRole('textbox')).toHaveValue('My custom guide')
  })

  it('Replace in template confirm replaces text', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'My custom guide' } })
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    fireEvent.click(screen.getByText(/Professional & direct/i))
    fireEvent.click(screen.getByRole('button', { name: /^Replace$/i }))
    expect(screen.getByRole('textbox')).not.toHaveValue('My custom guide')
  })

  it('applying a template does not auto-save', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.click(screen.getByRole('button', { name: /Templates/i }))
    fireEvent.click(screen.getByText(/Professional & direct/i))
    expect(mockInvoke).not.toHaveBeenCalledWith('save_project_voice_guide', expect.anything())
  })

  it('shows character count in X / LIMIT format', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByTestId('char-count'))
    expect(screen.getByTestId('char-count')).toHaveTextContent('11 / 5000')
  })
})

// ── Over-warn threshold ────────────────────────────────────────────────────────

describe('VoiceGuideBlock — over-warn threshold', () => {
  it('shows warning text when char count exceeds 2500', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(2501) } })
    expect(screen.getByText(/reduce generation quality/i)).toBeInTheDocument()
  })

  it('warning is absent below 2500 chars', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(100) } })
    expect(screen.queryByText(/reduce generation quality/i)).not.toBeInTheDocument()
  })

  it('warning is absent when over the hard limit (danger state takes over)', async () => {
    render(<VoiceGuideBlock projectId="proj-1" isOwner={true} />)
    await waitFor(() => screen.getByRole('textbox'))
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(5001) } })
    expect(screen.queryByText(/reduce generation quality/i)).not.toBeInTheDocument()
  })
})

// ── Template content validation (AI-M6) ───────────────────────────────────────

import { TEMPLATES } from './VoiceGuideBlock'

const FORBIDDEN_PHRASES = [
  'excited to share',
  'thrilled to announce',
  'game-changing',
  'revolutionary',
  'groundbreaking',
  'dive into',
  'delve into',
  'leverage',
  'seamlessly',
  'the future of',
  "i'm proud to",
  "i'm humbled to",
]

describe('VoiceGuideBlock — TEMPLATES must not contain forbidden phrases (AI-M6)', () => {
  for (const template of TEMPLATES) {
    it(`template "${template.label}" contains no forbidden phrases`, () => {
      const lower = template.text.toLowerCase()
      for (const phrase of FORBIDDEN_PHRASES) {
        expect(lower, `"${template.label}" contains "${phrase}"`).not.toContain(phrase)
      }
    })
  }
})
