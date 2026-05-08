// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import Wizard from './Wizard';

beforeEach(() => { vi.clearAllMocks(); });

describe('Wizard', () => {
  it('test_renders_modal_welcome_on_step_1', () => {
    render(<Wizard onComplete={vi.fn()} />);
    expect(screen.getByText('Welcome to Postlane')).toBeDefined();
  });

  it('test_renders_modal_account_on_step_2', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    expect(screen.getByText('Sign in to Postlane')).toBeDefined();
  });

  it('test_renders_modal_complete_on_step_5', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByRole('button', { name: /continue/i })).toBeDefined();
  });

  it('test_step_5_label_is_complete', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByText(/complete/i)).toBeDefined();
  });
});
