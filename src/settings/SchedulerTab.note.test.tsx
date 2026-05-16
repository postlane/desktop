// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab from './SchedulerTab';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(null);
});

describe('SchedulerTab — scheduler heading and description', () => {
  it('shows "Scheduler" as the section heading', () => {
    render(<SchedulerTab />);
    expect(screen.getByRole('heading', { name: /^Scheduler$/i })).toBeInTheDocument();
  });

  it('does not show a "Default scheduler" heading', () => {
    render(<SchedulerTab />);
    expect(screen.queryByText(/default scheduler/i)).not.toBeInTheDocument();
  });

  it('does not show per-repo override instructions', () => {
    render(<SchedulerTab />);
    expect(screen.queryByText(/configure per-repo/i)).not.toBeInTheDocument();
  });
});
