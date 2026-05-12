// SPDX-License-Identifier: BUSL-1.1
// Tests for 20.6.3 — wizard step 5: Install GitHub App button + deep link.
// All tests must be RED before any implementation is written.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';
import ModalGitHubApp from './ModalGitHubApp';

const mockOpenUrl = vi.mocked(openUrl);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  vi.clearAllMocks();
  mockListen.mockResolvedValue(() => {});
});

// ---------------------------------------------------------------------------
// Structure
// ---------------------------------------------------------------------------

describe('ModalGitHubApp — structure (20.6.3)', () => {
  it('renders an "Install GitHub App" button', () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByRole('button', { name: /install github app/i })).toBeDefined();
  });

  it('renders step 5 of 6 in WizardShell', () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByText(/5\s*\/\s*6|step 5/i)).toBeDefined();
  });

  it('has a heading describing repo connection', () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByRole('heading', { name: /connect repos/i })).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// GitHub provider — primary path
// ---------------------------------------------------------------------------

describe('ModalGitHubApp — GitHub provider (20.6.3)', () => {
  it('clicking Install GitHub App opens the app install URL in the browser', async () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /install github app/i }));
    expect(mockOpenUrl).toHaveBeenCalledOnce();
    const [url] = mockOpenUrl.mock.calls[0] as [string];
    expect(url).toMatch(/^https:\/\/github\.com\/apps\//);
  });

  it('Install button is not hidden for GitHub provider', () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    const btn = screen.getByRole('button', { name: /install github app/i });
    expect(btn).toBeDefined();
  });
});

// ---------------------------------------------------------------------------
// Non-GitHub provider — CLI fallback primary
// ---------------------------------------------------------------------------

describe('ModalGitHubApp — non-GitHub provider (20.6.7 preparation)', () => {
  it('shows CLI fallback disclosure for GitLab provider', () => {
    render(<ModalGitHubApp provider="gitlab" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByText('npx @postlane/cli init')).toBeDefined();
  });

  it('Install GitHub App button is hidden for GitLab provider', () => {
    render(<ModalGitHubApp provider="gitlab" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /install github app/i })).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

describe('ModalGitHubApp — navigation (20.6.3)', () => {
  it('onBack is called when Back is clicked', () => {
    const onBack = vi.fn();
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={onBack} />);
    fireEvent.click(screen.getByRole('button', { name: /back/i }));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it('shows a Skip option to advance without installing the app', () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    expect(screen.getByRole('button', { name: /skip/i })).toBeDefined();
  });

  it('onNext is called when Skip is clicked', () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp provider="github" onNext={onNext} onBack={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /skip/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });
});

// ---------------------------------------------------------------------------
// Deep link callback — wizard advancement (20.6.14 / 20.6.17 / 20.6.18)
// ---------------------------------------------------------------------------

describe('ModalGitHubApp — deep link callback (20.6.14)', () => {
  it('registers a listener for github:app-installed on mount (20.6.17)', async () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function));
    });
  });

  it('registers a listener for github:install-error on mount (20.6.18)', async () => {
    render(<ModalGitHubApp provider="github" onNext={vi.fn()} onBack={vi.fn()} />);
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('github:install-error', expect.any(Function));
    });
  });

  it('calls onNext when github:app-installed fires — wizard advances to step 6 (20.6.17)', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp provider="github" onNext={onNext} onBack={vi.fn()} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 12345 } }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('shows an inline error when github:install-error fires — wizard does not advance (20.6.18)', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp provider="github" onNext={onNext} onBack={vi.fn()} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:install-error', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:install-error');
    if (!entry) throw new Error('github:install-error listener not registered');
    act(() => (entry[1] as (e: { payload: { message: string } }) => void)({ payload: { message: 'Installation not found' } }));
    expect(screen.getByRole('alert')).toBeDefined();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('does not call onNext for non-GitHub provider when install event fires', async () => {
    const onNext = vi.fn();
    render(<ModalGitHubApp provider="gitlab" onNext={onNext} onBack={vi.fn()} />);
    await waitFor(() => expect(mockListen).toHaveBeenCalledWith('github:app-installed', expect.any(Function)));
    const entry = mockListen.mock.calls.find(([ev]) => ev === 'github:app-installed');
    if (!entry) throw new Error('github:app-installed listener not registered');
    act(() => (entry[1] as (e: { payload: { installation_id: number } }) => void)({ payload: { installation_id: 12345 } }));
    expect(onNext).not.toHaveBeenCalled();
  });
});
