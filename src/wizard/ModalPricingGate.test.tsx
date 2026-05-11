// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);

import ModalPricingGate from './ModalPricingGate';

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalPricingGate', () => {
  it('test_subscribe_opens_billing_url', async () => {
    mockInvoke.mockResolvedValue('none');
    render(<ModalPricingGate onPaid={vi.fn()} onBack={vi.fn()} pollIntervalMs={50} maxAttempts={1} />);
    await userEvent.click(screen.getByRole('button', { name: /subscribe/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/billing');
  });

  it('test_polling_calls_check_billing_gate', async () => {
    mockInvoke.mockResolvedValue('none');
    render(<ModalPricingGate onPaid={vi.fn()} onBack={vi.fn()} pollIntervalMs={30} maxAttempts={2} />);
    await userEvent.click(screen.getByRole('button', { name: /subscribe/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('check_billing_gate');
    }, { timeout: 3000 });
  });

  it('test_advances_when_gate_returns_free', async () => {
    mockInvoke.mockResolvedValue('free');
    const onPaid = vi.fn();
    render(<ModalPricingGate onPaid={onPaid} onBack={vi.fn()} pollIntervalMs={30} maxAttempts={5} />);
    await userEvent.click(screen.getByRole('button', { name: /subscribe/i }));
    await waitFor(() => expect(onPaid).toHaveBeenCalledOnce(), { timeout: 3000 });
  });

  it('test_check_again_shown_after_timeout', async () => {
    mockInvoke.mockResolvedValue('none');
    render(<ModalPricingGate onPaid={vi.fn()} onBack={vi.fn()} pollIntervalMs={30} maxAttempts={2} />);
    await userEvent.click(screen.getByRole('button', { name: /subscribe/i }));
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /check again/i })).toBeDefined();
    }, { timeout: 3000 });
  });

  it('test_back_button_stops_polling', async () => {
    mockInvoke.mockResolvedValue('none');
    const onBack = vi.fn();
    render(<ModalPricingGate onPaid={vi.fn()} onBack={onBack} pollIntervalMs={30} maxAttempts={100} />);
    await userEvent.click(screen.getByRole('button', { name: /subscribe/i }));
    // wait for polling to start (at least one invoke call)
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled(), { timeout: 500 });
    await userEvent.click(screen.getByRole('button', { name: /← back/i }));
    expect(onBack).toHaveBeenCalledOnce();
    const countAfterBack = mockInvoke.mock.calls.length;
    // interval must not fire after back is clicked
    await new Promise(r => setTimeout(r, 120));
    expect(mockInvoke.mock.calls.length).toBe(countAfterBack);
  });
});
