// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import LeftNav from './LeftNav';

vi.mock('../context/ProjectsProvider', () => ({
  useProjectsContext: () => ({ projects: [], loading: false, error: null, refresh: vi.fn() }),
}));
vi.mock('../context/DraftPostsProvider', () => ({
  useDraftPostsContext: () => ({ drafts: [] }),
}));
vi.mock('../hooks/useNavPersistence', () => ({
  useNavPersistence: () => () => {},
}));

describe('LeftNav CTA label', () => {
  it('renders "+ New workspace", not "+ Add org"', () => {
    render(
      <LeftNav
        onNavigate={vi.fn()}
        onSettingsOpen={vi.fn()}
        currentView={{ view: 'global_settings', section: 'account' }}
      />,
    );
    const buttonTexts = screen.getAllByRole('button').map((b) => b.textContent ?? '');
    expect(buttonTexts.some((t) => t.includes('New workspace'))).toBe(true);
    expect(buttonTexts.every((t) => !t.includes('Add org'))).toBe(true);
  });
});
