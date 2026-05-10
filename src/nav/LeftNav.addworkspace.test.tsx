// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('../context/ProjectsProvider', () => ({ useProjectsContext: vi.fn() }));
vi.mock('../context/DraftPostsProvider', () => ({ useDraftPostsContext: vi.fn() }));
vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn().mockReturnValue({
    outerSize: vi.fn().mockResolvedValue({ width: 1100, height: 700 }),
    outerPosition: vi.fn().mockResolvedValue({ x: 0, y: 0 }),
  }),
}));

import { useProjectsContext } from '../context/ProjectsProvider';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import LeftNav from './LeftNav';

const DEFAULT_VIEW = { view: 'no_orgs' as const };

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(useProjectsContext).mockReturnValue({
    projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  });
  vi.mocked(useDraftPostsContext).mockReturnValue({
    drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  });
});

describe('LeftNav — Add org button (§17.3 migrated)', () => {
  it('shows disabled Add org button with v1.2 tooltip when onAddWorkspace provided', () => {
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddWorkspace={vi.fn()}
      />,
    );
    const btn = screen.getByRole('button', { name: /add.*org/i });
    expect(btn).toBeInTheDocument();
    expect(btn).toHaveAttribute('title', expect.stringMatching(/v1\.2/i));
  });

  it('Add org button is disabled — v1.2 feature not yet active', () => {
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddWorkspace={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: /add.*org/i })).toBeDisabled();
  });
});
