// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest'
import { deriveOrgColour } from './orgColour'

describe('deriveOrgColour', () => {
  it('returns a valid HSL string', () => {
    expect(deriveOrgColour('org-1')).toMatch(/^hsl\(\d+, 65%, 55%\)$/)
  })

  it('is deterministic — same id returns same colour', () => {
    expect(deriveOrgColour('org-abc')).toBe(deriveOrgColour('org-abc'))
  })

  it('different ids produce different colours', () => {
    expect(deriveOrgColour('org-1')).not.toBe(deriveOrgColour('org-2'))
  })

  it('handles empty string without throwing', () => {
    expect(() => deriveOrgColour('')).not.toThrow()
    expect(deriveOrgColour('')).toMatch(/^hsl\(\d+, 65%, 55%\)$/)
  })
})
