// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { type ReactNode } from 'react';
import type { DraftPost, Project } from './types';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

vi.mock('./context/DraftPostsProvider', () => ({
  useDraftPostsContext: vi.fn(),
  DraftPostsProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));
vi.mock('./context/ProjectsProvider', () => ({
  useProjectsContext: vi.fn(),
  ProjectsProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));
vi.mock('./hooks/useSentPosts', () => ({
  useSentPosts: vi.fn(),
}));

vi.mock('./components/PostTable', () => ({
  default: ({ isHistory, onSelect }: { isHistory?: boolean; onSelect?: (_p: unknown) => void }) => (
    <div data-testid={isHistory ? 'PostTable-history' : 'PostTable-queue'}>
      {!isHistory && (
        <button data-testid="select-post" onClick={() => onSelect?.({ post_folder: 'post-001' })}>
          Select
        </button>
      )}
    </div>
  ),
}));
vi.mock('./settings/OrgSettingsView', () => ({
  default: () => <div data-testid="OrgSettingsView">OrgSettingsView</div>,
}));
vi.mock('./settings/AccountSettingsView', () => ({
  default: () => <div data-testid="AccountSettingsView">AccountSettingsView</div>,
}));
vi.mock('./settings/PreferencesSettingsView', () => ({
  default: () => <div data-testid="PreferencesSettingsView">PreferencesSettingsView</div>,
}));
vi.mock('./settings/SystemSettingsView', () => ({
  default: () => <div data-testid="SystemSettingsView">SystemSettingsView</div>,
}));
vi.mock('./components/EditPostView', () => ({
  default: ({ onDirtyChange }: { onDirtyChange?: (_d: boolean) => void }) => (
    <div data-testid="EditPostView">
      <button data-testid="make-dirty" onClick={() => onDirtyChange?.(true)}>
        Make dirty
      </button>
    </div>
  ),
}));
vi.mock('./TimezoneContext', () => ({
  useTimezone: vi.fn().mockReturnValue('UTC'),
  TimezoneContext: {
    Provider: ({ children }: { children: ReactNode }) => <>{children}</>,
  },
}));

import { MainContent } from './App';
import { useDraftPostsContext } from './context/DraftPostsProvider';
import { useProjectsContext } from './context/ProjectsProvider';
import { useSentPosts } from './hooks/useSentPosts';

function makeDraft(overrides: Partial<DraftPost> = {}): DraftPost {
  return {
    repo_id: 'r1', repo_name: 'my-repo', repo_path: '/repo',
    post_folder: 'post-001', platform: 'x', text: 'Hello',
    status: 'ready', platforms: ['x'], platform_results: null,
    schedule: null, llm_model: null, created_at: '2026-01-01T00:00:00Z',
    trigger: null, error: null, image_url: null, project_id: 'p1',
    ...overrides,
  };
}

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'p1', name: 'My Org', workspace_type: 'personal',
    tier: 'free', billing_active: true, is_owner: true,
    ...overrides,
  };
}

const defaultProps = {
  onNavigate: vi.fn(),
  onToast: vi.fn(),
  onDirtyChange: vi.fn(),
  onTimezoneChange: vi.fn(),
  onRepoChange: vi.fn(),
  onSignedOut: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(useDraftPostsContext).mockReturnValue({
    drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  });
  vi.mocked(useProjectsContext).mockReturnValue({
    projects: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
  });
  vi.mocked(useSentPosts).mockReturnValue({
    posts: [], loading: false, error: null, refresh: vi.fn(),
  });
});

// ---------------------------------------------------------------------------
// Routing dispatch
// ---------------------------------------------------------------------------

describe('MainContent routing — org_queue', () => {
  it('renders PostTable in queue mode', () => {
    render(<MainContent view={{ view: 'org_queue', projectId: 'p1' }} {...defaultProps} />);
    expect(screen.getByTestId('PostTable-queue')).toBeInTheDocument();
  });
});

describe('MainContent routing — org_history', () => {
  it('renders PostTable with isHistory=true', () => {
    render(<MainContent view={{ view: 'org_history', projectId: 'p1' }} {...defaultProps} />);
    expect(screen.getByTestId('PostTable-history')).toBeInTheDocument();
  });

  it('passes projectId to useSentPosts', () => {
    render(<MainContent view={{ view: 'org_history', projectId: 'proj-99' }} {...defaultProps} />);
    expect(vi.mocked(useSentPosts)).toHaveBeenCalledWith('proj-99');
  });
});

describe('MainContent routing — org_settings', () => {
  it('renders OrgSettingsView when project is found', () => {
    vi.mocked(useProjectsContext).mockReturnValue({
      projects: [makeProject()], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    });
    render(<MainContent view={{ view: 'org_settings', projectId: 'p1', section: 'queue' }} {...defaultProps} />);
    expect(screen.getByTestId('OrgSettingsView')).toBeInTheDocument();
  });
});

describe('MainContent routing — global_settings', () => {
  it('renders AccountSettingsView for section=account', () => {
    render(<MainContent view={{ view: 'global_settings', section: 'account' }} {...defaultProps} />);
    expect(screen.getByTestId('AccountSettingsView')).toBeInTheDocument();
  });

  it('renders PreferencesSettingsView for section=preferences', () => {
    render(<MainContent view={{ view: 'global_settings', section: 'preferences' }} {...defaultProps} />);
    expect(screen.getByTestId('PreferencesSettingsView')).toBeInTheDocument();
  });

  it('renders SystemSettingsView for section=system', () => {
    render(<MainContent view={{ view: 'global_settings', section: 'system' }} {...defaultProps} />);
    expect(screen.getByTestId('SystemSettingsView')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Post row click → EditPostView
// ---------------------------------------------------------------------------

describe('MainContent — post row click', () => {
  it('shows EditPostView when a queue post is selected', async () => {
    vi.mocked(useDraftPostsContext).mockReturnValue({
      drafts: [makeDraft()], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    });
    vi.mocked(useProjectsContext).mockReturnValue({
      projects: [makeProject()], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    });
    render(<MainContent view={{ view: 'org_queue', projectId: 'p1' }} {...defaultProps} />);
    fireEvent.click(screen.getByTestId('select-post'));
    await waitFor(() => expect(screen.getByTestId('EditPostView')).toBeInTheDocument());
  });
});

// ---------------------------------------------------------------------------
// Queue error state
// ---------------------------------------------------------------------------

describe('MainContent — queue error', () => {
  it('shows inline error with Retry button when drafts fail to load', () => {
    vi.mocked(useDraftPostsContext).mockReturnValue({
      drafts: [], loading: false, error: 'Network error', refresh: vi.fn(), clear: vi.fn(),
    });
    render(<MainContent view={{ view: 'org_queue', projectId: 'p1' }} {...defaultProps} />);
    expect(screen.getByText(/network error/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// onDirtyChange propagation
// ---------------------------------------------------------------------------

describe('MainContent — dirty state propagation', () => {
  it('calls onDirtyChange(true) when EditPostView becomes dirty', async () => {
    vi.mocked(useDraftPostsContext).mockReturnValue({
      drafts: [makeDraft()], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    });
    vi.mocked(useProjectsContext).mockReturnValue({
      projects: [makeProject()], loading: false, error: null, refresh: vi.fn(), clear: vi.fn(),
    });
    const onDirtyChange = vi.fn();
    render(<MainContent view={{ view: 'org_queue', projectId: 'p1' }} {...defaultProps} onDirtyChange={onDirtyChange} />);
    fireEvent.click(screen.getByTestId('select-post'));
    await waitFor(() => screen.getByTestId('EditPostView'));
    fireEvent.click(screen.getByTestId('make-dirty'));
    expect(onDirtyChange).toHaveBeenCalledWith(true);
  });
});
