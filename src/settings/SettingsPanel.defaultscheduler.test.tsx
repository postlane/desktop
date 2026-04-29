// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import SettingsPanel from './SettingsPanel';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(null);
});

describe('SettingsPanel — Default scheduler tab label (§15.1.1)', () => {
  it('renders the scheduler tab as "Default scheduler"', () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.getByRole('tab', { name: /default scheduler/i })).toBeInTheDocument();
  });

  it('does not render a tab with the bare label "scheduler"', () => {
    render(<SettingsPanel onClose={vi.fn()} />);
    expect(screen.queryByRole('tab', { name: /^scheduler$/i })).not.toBeInTheDocument();
  });
});
