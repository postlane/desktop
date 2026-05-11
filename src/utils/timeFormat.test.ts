// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest'
import { formatRelativeTime, formatScheduled } from './timeFormat'

describe('formatRelativeTime', () => {
  it('returns empty string for null', () => {
    expect(formatRelativeTime(null)).toBe('')
  })

  it('returns empty string for undefined', () => {
    expect(formatRelativeTime(undefined)).toBe('')
  })

  it('returns "just now" when less than 60 seconds ago', () => {
    const now = new Date('2024-06-01T12:00:30Z')
    const isoStr = '2024-06-01T12:00:00Z'
    expect(formatRelativeTime(isoStr, now)).toBe('just now')
  })

  it('returns minutes ago when less than 60 minutes', () => {
    const now = new Date('2024-06-01T12:05:00Z')
    const isoStr = '2024-06-01T12:00:00Z'
    expect(formatRelativeTime(isoStr, now)).toBe('5m ago')
  })

  it('returns hours ago when less than 24 hours', () => {
    const now = new Date('2024-06-01T15:00:00Z')
    const isoStr = '2024-06-01T12:00:00Z'
    expect(formatRelativeTime(isoStr, now)).toBe('3h ago')
  })

  it('returns days ago for older timestamps', () => {
    const now = new Date('2024-06-03T12:00:00Z')
    const isoStr = '2024-06-01T12:00:00Z'
    expect(formatRelativeTime(isoStr, now)).toBe('2d ago')
  })

  it('uses current time as default when now is not provided', () => {
    const past = new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString()
    expect(formatRelativeTime(past)).toBe('2h ago')
  })
})

describe('formatScheduled', () => {
  it('returns a string prefixed with "Scheduled for"', () => {
    const result = formatScheduled('2024-06-03T09:00:00Z', 'UTC')
    expect(result).toMatch(/^Scheduled for /)
  })

  it('includes the day in the output', () => {
    const result = formatScheduled('2024-06-03T09:00:00Z', 'UTC')
    expect(result).toMatch(/Jun|June|Mon|3/)
  })

  it('includes the time in the output', () => {
    const result = formatScheduled('2024-06-03T09:00:00Z', 'UTC')
    expect(result).toMatch(/9|09/)
  })

  it('respects the provided timezone', () => {
    const utcResult = formatScheduled('2024-06-03T09:00:00Z', 'UTC')
    const nyResult = formatScheduled('2024-06-03T09:00:00Z', 'America/New_York')
    expect(utcResult).not.toBe(nyResult)
  })

  it('returns empty string when isoStr is empty (not "Invalid Date")', () => {
    expect(formatScheduled('', 'UTC')).toBe('')
  })
})
