// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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

describe('LeftNav — Add org button', () => {
  it('test_add_org_button_is_enabled', () => {
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddWorkspace={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: /add.*workspace/i })).not.toBeDisabled();
  });

  it('test_add_org_button_calls_onAddWorkspace_when_clicked', async () => {
    const onAddWorkspace = vi.fn();
    render(
      <LeftNav
        currentView={DEFAULT_VIEW}
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        onAddWorkspace={onAddWorkspace}
      />,
    );
    await userEvent.setup().click(screen.getByRole('button', { name: /add.*workspace/i }));
    expect(onAddWorkspace).toHaveBeenCalledOnce();
  });
});
