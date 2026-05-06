// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import React from 'react';
import { TimezoneContext } from '../TimezoneContext';
import { ScheduleRow } from './ScheduleRow';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
const mockInvoke = vi.mocked(invoke);
const mockConfirm = vi.mocked(confirm);

beforeEach(() => vi.clearAllMocks());

function renderWithTz(tz: string, props: React.ComponentProps<typeof ScheduleRow>) {
  return render(
    <TimezoneContext.Provider value={tz}>
      <ScheduleRow {...props} />
    </TimezoneContext.Provider>,
  );
}

describe('ScheduleRow — timezone preservation (§review-product-low)', () => {
  it('passes timezone to update_post_schedule when setting a time', async () => {
    mockInvoke.mockResolvedValue(undefined);
    renderWithTz('America/New_York', {
      repoPath: '/repo',
      postFolder: 'post-001',
      schedule: null,
      onScheduleChange: vi.fn(),
    });
    fireEvent.click(screen.getByText(/\+ add time/i));
    const input = screen.getByLabelText(/scheduled time/i);
    fireEvent.change(input, { target: { value: '2026-06-15T10:00' } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'update_post_schedule',
        expect.objectContaining({ timezone: 'America/New_York' }),
      ),
    );
  });

  it('passes timezone when clearing a schedule', async () => {
    mockConfirm.mockResolvedValue(true);
    mockInvoke.mockResolvedValue(undefined);
    renderWithTz('Europe/London', {
      repoPath: '/repo',
      postFolder: 'post-001',
      schedule: '2026-06-15T14:00:00Z',
      onScheduleChange: vi.fn(),
    });
    await waitFor(() => screen.getByLabelText(/clear schedule/i));
    fireEvent.click(screen.getByLabelText(/clear schedule/i));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'update_post_schedule',
        expect.objectContaining({ timezone: 'Europe/London' }),
      ),
    );
  });
});
