// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import LeftNav from './LeftNav';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../hooks/useRepoData', () => ({
  useRepoData: () => ({ repos: [], loadError: null, refresh: vi.fn() }),
}));
vi.mock('../hooks/useAppStateRestore', () => ({ useAppStateRestore: vi.fn() }));
vi.mock('../hooks/useMetaChangedListener', () => ({ useMetaChangedListener: () => null }));
vi.mock('../hooks/useWatcherHealth', () => ({ useWatcherHealth: () => new Set() }));
vi.mock('../hooks/useNavPersistence', () => ({ useNavPersistence: () => vi.fn() }));

const DEFAULT_VIEW = { view: 'all_repos' as const, repoId: null, section: 'drafts' as const };

beforeEach(() => vi.clearAllMocks());

describe('LeftNav — add workspace button (§17.3)', () => {
  it('renders the add workspace button with correct aria-label', () => {
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddRepo={vi.fn()}
        onAddWorkspace={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: /add workspace/i })).toBeInTheDocument();
  });

  it('calls onAddWorkspace when the button is clicked', () => {
    const onAddWorkspace = vi.fn();
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddRepo={vi.fn()}
        onAddWorkspace={onAddWorkspace}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /add workspace/i }));
    expect(onAddWorkspace).toHaveBeenCalledOnce();
  });

  it('does not render the add workspace button when onAddWorkspace is not provided', () => {
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddRepo={vi.fn()}
      />,
    );
    expect(screen.queryByRole('button', { name: /add workspace/i })).not.toBeInTheDocument();
  });
});
