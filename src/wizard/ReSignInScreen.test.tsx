// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

import ReSignInScreen from './ReSignInScreen';

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(false);
});

describe('ReSignInScreen', () => {
  it('test_renders_sign_in_message', () => {
    render(<ReSignInScreen onSignedIn={vi.fn()} pollIntervalMs={10000} />);
    expect(screen.getByText(/session has expired/i)).toBeDefined();
  });

  it('test_calls_onSignedIn_when_token_detected', async () => {
    mockInvoke.mockResolvedValue(true);
    const onSignedIn = vi.fn();
    render(<ReSignInScreen onSignedIn={onSignedIn} pollIntervalMs={30} />);
    await waitFor(() => expect(onSignedIn).toHaveBeenCalledOnce(), { timeout: 3000 });
  });
});
