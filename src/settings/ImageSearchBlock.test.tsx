// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import ImageSearchBlock from './ImageSearchBlock';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

function noKeyInvoke() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'has_unsplash_key') return Promise.resolve(false);
    return Promise.resolve(null);
  });
}

function hasKeyInvoke() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'has_unsplash_key') return Promise.resolve(true);
    return Promise.resolve(null);
  });
}

beforeEach(() => { vi.clearAllMocks(); noKeyInvoke(); });

// ── No key ────────────────────────────────────────────────────────────────────

describe('ImageSearchBlock — no key', () => {
  it('renders an "Image search" heading', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByText(/image search/i)).toBeInTheDocument();
  });

  it('shows an Unsplash label', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByText('Unsplash')).toBeInTheDocument();
  });

  it('shows a Connect button', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByRole('button', { name: /^connect$/i })).toBeInTheDocument();
  });

  it('does not show the key input by default', async () => {
    render(<ImageSearchBlock />);
    await screen.findByText('Unsplash');
    expect(screen.queryByRole('textbox', { name: /unsplash access key/i })).not.toBeInTheDocument();
  });

  it('shows an external link button for Unsplash', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByRole('button', { name: /open unsplash website/i })).toBeInTheDocument();
  });
});

// ── Connect flow ──────────────────────────────────────────────────────────────

describe('ImageSearchBlock — connect flow', () => {
  it('clicking Connect expands the key input form', async () => {
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    expect(await screen.findByLabelText(/unsplash access key/i)).toBeInTheDocument();
  });

  it('form has Show, Connect, and Cancel buttons', async () => {
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    expect(await screen.findByRole('button', { name: /show/i })).toBeInTheDocument();
    const connectBtns = await screen.findAllByRole('button', { name: /^connect$/i });
    expect(connectBtns.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
  });

  it('Show toggles input type between password and text', async () => {
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    const input = await screen.findByLabelText(/unsplash access key/i);
    expect(input).toHaveAttribute('type', 'password');
    fireEvent.click(screen.getByRole('button', { name: /show/i }));
    expect(input).toHaveAttribute('type', 'text');
  });

  it('clicking Cancel hides the form', async () => {
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    await screen.findByLabelText(/unsplash access key/i);
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByLabelText(/unsplash access key/i)).not.toBeInTheDocument();
  });
});

describe('ImageSearchBlock — connect flow — save', () => {
  it('calls save_unsplash_key with trimmed key when Connect is clicked', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(false);
      if (cmd === 'save_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    const input = await screen.findByLabelText(/unsplash access key/i);
    fireEvent.change(input, { target: { value: '  my-test-key-abc  ' } });
    // Click the Connect button inside the form (the row Connect button is gone once expanded)
    const connectBtns = screen.getAllByRole('button', { name: /^connect$/i });
    fireEvent.click(connectBtns[connectBtns.length - 1]);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_unsplash_key', { accessKey: 'my-test-key-abc' }),
    );
  });

  it('does not call any file-write command when saving key', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(false);
      if (cmd === 'save_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    const input = await screen.findByLabelText(/unsplash access key/i);
    fireEvent.change(input, { target: { value: 'my-test-key-abc' } });
    const connectBtns = screen.getAllByRole('button', { name: /^connect$/i });
    fireEvent.click(connectBtns[connectBtns.length - 1]);
    await waitFor(() => {
      const allCalls = mockInvoke.mock.calls.map(([cmd]) => cmd);
      expect(allCalls).not.toContain('write_project_id_to_config');
      expect(allCalls).not.toContain('save_app_state_command');
    });
  });

  it('after successful connect shows Change key and Disconnect', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(false);
      if (cmd === 'save_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /^connect$/i }));
    const input = await screen.findByLabelText(/unsplash access key/i);
    fireEvent.change(input, { target: { value: 'my-test-key-abc' } });
    const connectBtns = screen.getAllByRole('button', { name: /^connect$/i });
    fireEvent.click(connectBtns[connectBtns.length - 1]);
    expect(await screen.findByRole('button', { name: /change key/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument();
  });
});

// ── Key configured ────────────────────────────────────────────────────────────

describe('ImageSearchBlock — key configured', () => {
  it('shows Change key and Disconnect when key is configured', async () => {
    hasKeyInvoke();
    render(<ImageSearchBlock />);
    expect(await screen.findByRole('button', { name: /change key/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument();
  });

  it('shows an external link button when key is configured', async () => {
    hasKeyInvoke();
    render(<ImageSearchBlock />);
    await screen.findByRole('button', { name: /change key/i });
    expect(screen.getByRole('button', { name: /open unsplash website/i })).toBeInTheDocument();
  });

  it('does not show a Connect button when key is configured', async () => {
    hasKeyInvoke();
    render(<ImageSearchBlock />);
    await screen.findByRole('button', { name: /change key/i });
    expect(screen.queryByRole('button', { name: /^connect$/i })).not.toBeInTheDocument();
  });

  it('clicking Change key shows the key input form', async () => {
    hasKeyInvoke();
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /change key/i }));
    expect(await screen.findByLabelText(/unsplash access key/i)).toBeInTheDocument();
  });

  it('calls delete_unsplash_key when Disconnect is clicked', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(true);
      if (cmd === 'delete_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_unsplash_key'),
    );
  });

  it('after disconnect shows Connect button', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(true);
      if (cmd === 'delete_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /disconnect/i }));
    expect(await screen.findByRole('button', { name: /^connect$/i })).toBeInTheDocument();
  });

  // 21.8.21: key stored via keyring command; not written to any file
  it('21.8.21 key is stored via keyring command not file write', async () => {
    hasKeyInvoke();
    render(<ImageSearchBlock />);
    await screen.findByRole('button', { name: /change key/i });
    const allCalls = mockInvoke.mock.calls.map(([cmd]) => cmd);
    expect(allCalls).not.toContain('write_project_id_to_config');
    expect(allCalls).not.toContain('save_app_state_command');
  });
});
