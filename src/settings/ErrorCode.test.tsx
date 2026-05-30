// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'
import '@testing-library/jest-dom'
import { invoke } from '@tauri-apps/api/core'
import { ErrorCode } from './ErrorCode'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

beforeEach(() => { vi.clearAllMocks() })

describe('ErrorCode (22.9.10b)', () => {
  it('renders the error code and message', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('1.4.2')
    render(<ErrorCode code="PL-WS-001" message="No repos found" />)
    expect(screen.getByTestId('error-code-label')).toBeInTheDocument()
    expect(screen.getByText(/No repos found/)).toBeInTheDocument()
  })

  it('appends app version to error code label', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('1.4.2')
    render(<ErrorCode code="PL-DEL-003" message="GitLab revocation skipped" />)
    await waitFor(() => {
      expect(screen.getByTestId('error-code-label')).toHaveTextContent('PL-DEL-003 · v1.4.2')
    })
  })

  it('renders code without version while version loads', () => {
    vi.mocked(invoke).mockReturnValue(new Promise(() => {}))
    render(<ErrorCode code="PL-MIG-001" message="Copy failed" />)
    expect(screen.getByTestId('error-code-label')).toHaveTextContent('PL-MIG-001')
    expect(screen.getByTestId('error-code-label')).not.toHaveTextContent('v')
  })

  it('reads version from get_app_version command', async () => {
    vi.mocked(invoke).mockResolvedValueOnce('2.0.0')
    render(<ErrorCode code="PL-WS-003" message="Is a repo" />)
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_app_version')
    })
  })
})
