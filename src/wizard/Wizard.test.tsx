// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import Wizard from './Wizard';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-clipboard-manager', () => ({
  writeText: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

const mockInvoke = vi.mocked(invoke);
const mockOpenDialog = vi.mocked(openDialog);
const mockWriteText = vi.mocked(writeText);

beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

function renderWizard(onComplete = vi.fn()) {
  return render(<Wizard onComplete={onComplete} />);
}

// ---------------------------------------------------------------------------
// Step 1: Add a repo
// ---------------------------------------------------------------------------

describe('Wizard — Step 1', () => {
  it('renders the opening question', () => {
    renderWizard();
    expect(
      screen.getByText(/have you already run/i),
    ).toBeInTheDocument();
  });

  it('shows the "Yes" and "No" options', () => {
    renderWizard();
    expect(screen.getByRole('button', { name: /yes/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /no/i })).toBeInTheDocument();
  });

  it('"No" branch shows the terminal command', () => {
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /no/i }));
    expect(screen.getByText(/npx postlane init/i)).toBeInTheDocument();
  });

  it('"No" branch shows "Open terminal guide →" link', () => {
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /no/i }));
    expect(screen.getByRole('link', { name: /open terminal guide/i })).toBeInTheDocument();
  });

  it('"Yes" branch opens folder picker', async () => {
    mockOpenDialog.mockResolvedValue(null);
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    expect(mockOpenDialog).toHaveBeenCalledWith({ directory: true });
  });

  it('shows error when selected folder has no config.json', async () => {
    mockOpenDialog.mockResolvedValue('/some/path');
    mockInvoke.mockRejectedValue(new Error('config.json not found'));
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    await waitFor(() =>
      expect(screen.getByText(/run `postlane init` inside it first/i)).toBeInTheDocument(),
    );
  });

  it('advances to step 2 when add_repo succeeds', async () => {
    mockOpenDialog.mockResolvedValue('/valid/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'add_repo') return { id: 'r1', name: 'valid', path: '/valid/repo', active: true, added_at: '' };
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      return null;
    });
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    await waitFor(() =>
      expect(screen.getByText(/connect a scheduler/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Step 2: Connect a scheduler
// ---------------------------------------------------------------------------

describe('Wizard — Step 2', () => {
  async function goToStep2(credentialExists: boolean) {
    mockOpenDialog.mockResolvedValue('/valid/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'add_repo') return { id: 'r1', name: 'my-repo', path: '/valid/repo', active: true, added_at: '' };
      if (cmd === 'get_scheduler_credential') {
        if (credentialExists) return 'sk-••••abcd';
        throw new Error('not found');
      }
      return null;
    });
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    await screen.findByText(/connect a scheduler/i);
  }

  it('shows connected state when credential exists', async () => {
    await goToStep2(true);
    expect(screen.getByText(/connected/i)).toBeInTheDocument();
  });

  it('shows provider selector when no credential exists', async () => {
    await goToStep2(false);
    expect(screen.getByRole('combobox')).toBeInTheDocument();
  });

  it('"Skip for now" advances to step 3', async () => {
    await goToStep2(false);
    fireEvent.click(screen.getByRole('button', { name: /skip for now/i }));
    await waitFor(() =>
      expect(screen.getByText(/you're ready/i)).toBeInTheDocument(),
    );
  });

  it('"Continue" with existing credential advances to step 3', async () => {
    await goToStep2(true);
    fireEvent.click(screen.getByRole('button', { name: /continue/i }));
    await waitFor(() =>
      expect(screen.getByText(/you're ready/i)).toBeInTheDocument(),
    );
  });
});

// ---------------------------------------------------------------------------
// Step 3: You're ready
// ---------------------------------------------------------------------------

describe('Wizard — Step 3', () => {
  async function goToStep3() {
    mockOpenDialog.mockResolvedValue('/valid/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'add_repo') return { id: 'r1', name: 'my-repo', path: '/valid/repo', active: true, added_at: '' };
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      return null;
    });
    renderWizard();
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    await screen.findByText(/connect a scheduler/i);
    fireEvent.click(screen.getByRole('button', { name: /skip for now/i }));
    await screen.findByText(/you're ready/i);
  }

  it('shows the repo name registered in step 1', async () => {
    await goToStep3();
    expect(screen.getAllByText(/my-repo/).length).toBeGreaterThan(0);
  });

  it('shows the /draft-post command', async () => {
    await goToStep3();
    expect(screen.getByText('/draft-post')).toBeInTheDocument();
  });

  it('copy button writes /draft-post to clipboard', async () => {
    mockWriteText.mockResolvedValue(undefined);
    await goToStep3();
    fireEvent.click(screen.getByRole('button', { name: /copy/i }));
    expect(mockWriteText).toHaveBeenCalledWith('/draft-post');
  });

  it('"Done" calls onComplete', async () => {
    const onComplete = vi.fn();
    mockOpenDialog.mockResolvedValue('/valid/repo');
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'add_repo') return { id: 'r1', name: 'my-repo', path: '/valid/repo', active: true, added_at: '' };
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      return null;
    });
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByRole('button', { name: /yes/i }));
    const browse = await screen.findByRole('button', { name: /browse for the folder/i });
    fireEvent.click(browse);
    await screen.findByText(/connect a scheduler/i);
    fireEvent.click(screen.getByRole('button', { name: /skip for now/i }));
    await screen.findByText(/you're ready/i);
    fireEvent.click(screen.getByRole('button', { name: /done/i }));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it('clipboard fallback: selects text when writeText fails', async () => {
    mockWriteText.mockRejectedValue(new Error('clipboard unavailable'));
    await goToStep3();
    fireEvent.click(screen.getByRole('button', { name: /copy/i }));
    await waitFor(() =>
      expect(screen.getByText(/press ctrl\+c to copy/i)).toBeInTheDocument(),
    );
  });
});
