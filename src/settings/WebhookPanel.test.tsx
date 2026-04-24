// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import WebhookPanel from './WebhookPanel';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('WebhookPanel — URL input', () => {
  it('renders a text input (not password) for the webhook URL', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    expect(input).toHaveAttribute('type', 'url');
  });

  it('shows an inline error when http:// URL is entered', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    fireEvent.change(input, { target: { value: 'http://insecure.example.com/hook' } });
    await waitFor(() =>
      expect(screen.getByText(/must use https/i)).toBeInTheDocument(),
    );
  });

  it('does not show an error for a valid https:// URL', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    fireEvent.change(input, { target: { value: 'https://hooks.zapier.com/hooks/catch/abc' } });
    await waitFor(() =>
      expect(screen.queryByText(/must use https/i)).not.toBeInTheDocument(),
    );
  });

  it('does not save when URL starts with http://', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    fireEvent.change(input, { target: { value: 'http://insecure.example.com/hook' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    expect(mockInvoke).not.toHaveBeenCalledWith('save_scheduler_credential', expect.anything());
  });
});

describe('WebhookPanel — save', () => {
  it('calls save_scheduler_credential with provider webhook on valid save', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    fireEvent.change(input, { target: { value: 'https://hooks.zapier.com/hooks/catch/abc' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_scheduler_credential',
        expect.objectContaining({ provider: 'webhook', apiKey: 'https://hooks.zapier.com/hooks/catch/abc' }),
      ),
    );
  });
});

describe('WebhookPanel — configured state', () => {
  it('shows Test and Remove buttons when credential is configured', async () => {
    mockInvoke.mockResolvedValue('https://hooks.zapier.com/hooks/catch/abc');
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /test/i }));
    expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument();
  });

  it('calls test_scheduler with webhook when Test is clicked', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'https://hooks.zapier.com/hooks/catch/abc';
      if (cmd === 'test_scheduler') return true;
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /test/i }));
    fireEvent.click(screen.getByRole('button', { name: /test/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('test_scheduler', { provider: 'webhook' }),
    );
  });
});
