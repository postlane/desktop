// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab from './SchedulerTab';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(null);
});

describe('SchedulerTab — default credentials note (§15.1.2)', () => {
  it('shows the per-repo configure note below the heading', () => {
    render(<SchedulerTab />);
    expect(
      screen.getByText(/configure per-repo in settings.*repos.*configure/i),
    ).toBeInTheDocument();
  });

  it('does not show the old "v1.1" placeholder text', () => {
    render(<SchedulerTab />);
    expect(screen.queryByText(/configurable in v1\.1/i)).not.toBeInTheDocument();
  });
});
