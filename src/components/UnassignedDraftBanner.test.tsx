// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('../context/DraftPostsProvider', () => ({ useDraftPostsContext: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { useDraftPostsContext } from '../context/DraftPostsProvider';
import UnassignedDraftBanner from './UnassignedDraftBanner';

const mockInvoke = vi.mocked(invoke);
const mockUseDraftPostsContext = vi.mocked(useDraftPostsContext);

const UNASSIGNED_DRAFT = {
  repo_id: 'r1', repo_name: 'MyRepo', repo_path: '/p', post_folder: 'f',
  platforms: ['x'], platform: 'x', text: 'Hi', status: 'ready' as const,
  trigger: null, error: null, image_url: null, project_id: null,
  schedule: null, platform_results: null, llm_model: null, created_at: null,
};

const ASSIGNED_DRAFT = { ...UNASSIGNED_DRAFT, project_id: 'proj-1' };

beforeEach(() => {
  vi.clearAllMocks();
  mockUseDraftPostsContext.mockReturnValue({ drafts: [], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
});

describe('UnassignedDraftBanner', () => {
  it('renders nothing when all drafts are assigned', async () => {
    mockInvoke.mockResolvedValue({ dismissed_unassigned_draft_warning: false });
    mockUseDraftPostsContext.mockReturnValue({ drafts: [ASSIGNED_DRAFT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
    const { container } = render(<UnassignedDraftBanner />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(container.firstChild).toBeNull();
  });

  it('renders nothing when dismissed flag is true', async () => {
    mockInvoke.mockResolvedValue({ dismissed_unassigned_draft_warning: true });
    mockUseDraftPostsContext.mockReturnValue({ drafts: [UNASSIGNED_DRAFT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
    const { container } = render(<UnassignedDraftBanner />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());
    expect(container.firstChild).toBeNull();
  });

  it('shows banner when unassigned draft exists and not dismissed', async () => {
    mockInvoke.mockResolvedValue({ dismissed_unassigned_draft_warning: false });
    mockUseDraftPostsContext.mockReturnValue({ drafts: [UNASSIGNED_DRAFT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
    render(<UnassignedDraftBanner />);
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  });

  it('shows banner when initial load fails (catch sets dismissed=false)', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC error'));
    mockUseDraftPostsContext.mockReturnValue({ drafts: [UNASSIGNED_DRAFT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
    render(<UnassignedDraftBanner />);
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  });

  it('hides banner after Dismiss is clicked', async () => {
    mockInvoke.mockResolvedValue({ dismissed_unassigned_draft_warning: false });
    mockUseDraftPostsContext.mockReturnValue({ drafts: [UNASSIGNED_DRAFT], loading: false, error: null, refresh: vi.fn(), clear: vi.fn() });
    render(<UnassignedDraftBanner />);
    await waitFor(() => screen.getByRole('button', { name: /dismiss/i }));
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  });
});
