// SPDX-License-Identifier: BUSL-1.1
// Tests for 20.6.3 — wizard step 5: Install GitHub App button + deep link.
// All tests must be RED before any implementation is written.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

import { openUrl } from '@tauri-apps/plugin-opener';
import ModalGitHubApp from './ModalGitHubApp';

const mockOpenUrl = vi.mocked(openUrl);

beforeEach(() => {
  vi.clearAllMocks();
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
