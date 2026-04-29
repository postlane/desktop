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

describe('WebhookPanel — save error', () => {
  it('shows an error message when save_scheduler_credential fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') throw new Error('Keychain locked');
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const input = await screen.findByPlaceholderText(/https:\/\//i);
    fireEvent.change(input, { target: { value: 'https://hooks.zapier.com/hooks/catch/abc' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(screen.getByText(/keychain locked/i)).toBeInTheDocument(),
    );
  });

  it('shows masked URL in configured state (not full URL)', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'https://hooks.zapier.com/hooks/catch/abc123secret';
      if (cmd === 'get_scheduler_usage') return { provider: 'webhook', count: 0, limit: null, month: 4, year: 2026 };
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /test/i }));
    expect(screen.queryByText('https://hooks.zapier.com/hooks/catch/abc123secret')).not.toBeInTheDocument();
  });
});

describe('WebhookPanel — usage display (§13.1.3)', () => {
  it('shows webhook usage count when get_scheduler_usage returns data', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return { provider: 'webhook', count: 50, limit: 100, month: 4, year: 2026 };
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() =>
      expect(screen.getByText(/50\/100 posts used this month/i)).toBeInTheDocument(),
    );
  });

  it('shows zero usage when count is 0 and limit is known', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return { provider: 'webhook', count: 0, limit: 100, month: 4, year: 2026 };
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() =>
      expect(screen.getByText(/0\/100 posts used this month/i)).toBeInTheDocument(),
    );
  });

  it('does not show usage when limit is null', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return { provider: 'webhook', count: 0, limit: null, month: 4, year: 2026 };
      return null;
    });
    render(<WebhookPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    expect(screen.queryByText(/posts used this month/i)).not.toBeInTheDocument();
  });
});

describe('WebhookPanel — configured state', () => {
  it('shows Test and Remove buttons when credential is configured', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return 'https://hooks.zapier.com/hooks/catch/abc';
      if (cmd === 'get_scheduler_usage') return { provider: 'webhook', count: 0, limit: null, month: 4, year: 2026 };
      return null;
    });
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
