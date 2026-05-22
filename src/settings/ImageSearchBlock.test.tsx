// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import ImageSearchBlock from './ImageSearchBlock';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'has_unsplash_key') return Promise.resolve(false);
    return Promise.resolve(null);
  });
});

// 21.8.11: Image search settings section
describe('ImageSearchBlock', () => {
  it('renders an "Image search" heading', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByText(/image search/i)).toBeInTheDocument();
  });

  it('shows a key input field', async () => {
    render(<ImageSearchBlock />);
    expect(await screen.findByRole('textbox', { name: /unsplash access key/i })).toBeInTheDocument();
  });

  it('shows "Key configured" when has_unsplash_key returns true', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    expect(await screen.findByText(/key configured/i)).toBeInTheDocument();
  });

  // 21.8.21: key stored via keyring command; not written to any file
  it('calls save_unsplash_key when Save is clicked', async () => {
    render(<ImageSearchBlock />);
    const input = await screen.findByRole('textbox', { name: /unsplash access key/i });
    fireEvent.change(input, { target: { value: 'my-test-key-abc' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_unsplash_key', { accessKey: 'my-test-key-abc' });
    });
  });

  it('does not call any file-write command when saving key', async () => {
    render(<ImageSearchBlock />);
    const input = await screen.findByRole('textbox', { name: /unsplash access key/i });
    fireEvent.change(input, { target: { value: 'my-test-key-abc' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => {
      const allCalls = mockInvoke.mock.calls.map(([cmd]) => cmd);
      expect(allCalls).not.toContain('write_project_id_to_config');
      expect(allCalls).not.toContain('save_app_state_command');
    });
  });

  it('shows a Remove button when key is configured', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    expect(await screen.findByRole('button', { name: /remove/i })).toBeInTheDocument();
  });

  it('calls delete_unsplash_key when Remove is clicked', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'has_unsplash_key') return Promise.resolve(true);
      if (cmd === 'delete_unsplash_key') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    render(<ImageSearchBlock />);
    fireEvent.click(await screen.findByRole('button', { name: /remove/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_unsplash_key');
    });
  });
});
