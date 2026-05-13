// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SubstackNotesPanel from './SubstackNotesPanel';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('SubstackNotesPanel — persistent warnings', () => {
  it('shows the session expiry warning when not configured', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<SubstackNotesPanel />);
    await waitFor(() =>
      expect(screen.getByText(/session expires/i)).toBeInTheDocument(),
    );
  });

  it('shows the immediate-posting warning when not configured', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<SubstackNotesPanel />);
    await waitFor(() =>
      expect(screen.getByText(/always post immediately/i)).toBeInTheDocument(),
    );
  });

  it('shows both warnings when credential is configured', async () => {
    mockInvoke.mockResolvedValue('••••xyzw');
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByText(/session expires/i));
    expect(screen.getByText(/always post immediately/i)).toBeInTheDocument();
  });
});

describe('SubstackNotesPanel — credential input', () => {
  it('renders a textarea (not a password input) for cookie entry', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    await waitFor(() => expect(screen.getByRole('textbox')).toBeInTheDocument());
    expect(screen.queryByDisplayValue('')).not.toBeNull();
    const textarea = document.querySelector('textarea');
    expect(textarea).toBeInTheDocument();
  });

  it('calls save_scheduler_credential with provider substack_notes on save', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const textarea = await screen.findByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'my-session-cookie-abc' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_scheduler_credential',
        expect.objectContaining({ provider: 'substack_notes', apiKey: 'my-session-cookie-abc' }),
      ),
    );
  });
});

describe('SubstackNotesPanel — save error', () => {
  it('shows an error message when save_scheduler_credential fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'save_scheduler_credential') throw new Error('Keychain locked');
      return null;
    });
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    const textarea = await screen.findByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'my-cookie' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(screen.getByText(/keychain locked/i)).toBeInTheDocument(),
    );
  });
});

describe('SubstackNotesPanel — cancel behaviour', () => {
  it('cancel from adding with no credential returns to idle state', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /add/i }));
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /add/i })).toBeInTheDocument());
  });

  it('Change button from configured state switches to adding form', async () => {
    mockInvoke.mockResolvedValue('••••xyzw');
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /change/i }));
    fireEvent.click(screen.getByRole('button', { name: /change/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument());
  });

  it('cancel from adding with existing credential returns to configured state', async () => {
    mockInvoke.mockResolvedValue('••••xyzw');
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /change/i }));
    fireEvent.click(screen.getByRole('button', { name: /change/i }));
    await waitFor(() => screen.getByRole('button', { name: /cancel/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /change/i })).toBeInTheDocument());
  });
});

describe('SubstackNotesPanel — configured state', () => {
  it('shows Test and Remove buttons when credential is present', async () => {
    mockInvoke.mockResolvedValue('••••xyzw');
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /test/i }));
    expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument();
  });

  it('calls test_scheduler with substack_notes when Test is clicked', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return '••••xyzw';
      if (cmd === 'test_scheduler') return true;
      return null;
    });
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /test/i }));
    fireEvent.click(screen.getByRole('button', { name: /test/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('test_scheduler', { provider: 'substack_notes' }),
    );
  });

  it('shows error message when Remove fails (§review-silentcatch)', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') return '••••xyzw';
      if (cmd === 'delete_scheduler_credential') throw new Error('Keychain locked');
      return null;
    });
    render(<SubstackNotesPanel />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  });
});
