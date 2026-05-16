// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoConfigureModal from './RepoConfigureModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));
import { invoke } from '../ipc/invoke';
import { confirm } from '@tauri-apps/plugin-dialog';
const mockInvoke = vi.mocked(invoke);
const mockConfirm = vi.mocked(confirm);

beforeEach(() => vi.clearAllMocks());

function setupMocks(voiceGuide: string | null = null) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'get_project_voice_guide') return voiceGuide;
    if (cmd === 'save_project_voice_guide') return null;
    return null;
  });
}

function renderWithProject(voiceGuide: string | null = null) {
  setupMocks(voiceGuide);
  render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
}

describe('RepoConfigureModal — voice guide — display (§17.1)', () => {
  it('renders a voice guide textarea', async () => {
    renderWithProject(null);
    expect(await screen.findByRole('textbox', { name: /voice guide/i })).toBeInTheDocument();
  });

  it('textarea has placeholder mentioning "No voice guide set"', async () => {
    renderWithProject(null);
    const ta = await screen.findByRole('textbox', { name: /voice guide/i });
    expect(ta).toHaveAttribute('placeholder', expect.stringContaining('No voice guide set'));
  });

  it('pre-populates textarea with existing voice guide', async () => {
    renderWithProject('Direct and technical.');
    const ta = await screen.findByRole('textbox', { name: /voice guide/i });
    expect((ta as HTMLTextAreaElement).value).toBe('Direct and technical.');
  });

  it('textarea is disabled while loading', () => {
    let resolve: (v: string | null) => void = () => {};
    const pending = new Promise<string | null>((res) => { resolve = res; });
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_project_voice_guide') return pending;
      return null;
    });
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
    expect(screen.getByRole('textbox', { name: /voice guide/i })).toBeDisabled();
    resolve(null);
  });

  it('shows a hint about where the file is saved', async () => {
    renderWithProject(null);
    await screen.findByRole('textbox', { name: /voice guide/i });
    expect(screen.getByText(/voice-guide\.md/i)).toBeInTheDocument();
  });

  it('does not render voice guide section when projectId is absent', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.queryByRole('textbox', { name: /voice guide/i })).not.toBeInTheDocument();
  });
});

describe('RepoConfigureModal — voice guide — load failure (§fix-8)', () => {
  it('shows an error message when get_project_voice_guide fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_project_voice_guide') throw new Error('Network unreachable');
      return null;
    });
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText(/network unreachable/i)).toBeInTheDocument());
  });
});

describe('RepoConfigureModal — voice guide — template (§17.1)', () => {
  it('shows "Start from template" button when textarea is empty', async () => {
    renderWithProject(null);
    await screen.findByRole('textbox', { name: /voice guide/i });
    expect(screen.getByRole('button', { name: /start from template/i })).toBeInTheDocument();
  });

  it('does not show "Start from template" when textarea has content', async () => {
    renderWithProject('Direct and technical.');
    await screen.findByRole('textbox', { name: /voice guide/i });
    expect(screen.queryByRole('button', { name: /start from template/i })).not.toBeInTheDocument();
  });

  it('"Start from template" populates textarea with non-empty text', async () => {
    renderWithProject(null);
    fireEvent.click(await screen.findByRole('button', { name: /start from template/i }));
    const ta = screen.getByRole('textbox', { name: /voice guide/i }) as HTMLTextAreaElement;
    expect(ta.value).not.toBe('');
  });
});

describe('RepoConfigureModal — voice guide — save (§17.1)', () => {
  it('Save calls save_project_voice_guide with textarea content', async () => {
    renderWithProject('Direct and technical.');
    await screen.findByRole('textbox', { name: /voice guide/i });
    fireEvent.click(screen.getByRole('button', { name: /save voice guide/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', { projectId: 'proj-abc', voiceGuide: 'Direct and technical.' }),
    );
  });

  it('shows saved confirmation after successful save', async () => {
    renderWithProject('Direct and technical.');
    await screen.findByRole('textbox', { name: /voice guide/i });
    fireEvent.click(screen.getByRole('button', { name: /save voice guide/i }));
    await waitFor(() => expect(screen.getByText(/✓/)).toBeInTheDocument());
  });

  it('shows error inline when save fails; Save button remains', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_project_voice_guide') return 'some text';
      if (cmd === 'save_project_voice_guide') throw new Error('API error');
      return null;
    });
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
    await screen.findByRole('textbox', { name: /voice guide/i });
    fireEvent.click(screen.getByRole('button', { name: /save voice guide/i }));
    await waitFor(() => expect(screen.getByText(/api error/i)).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /save voice guide/i })).toBeInTheDocument();
  });

  it('saving empty voice guide asks for confirmation', async () => {
    mockConfirm.mockResolvedValue(false);
    setupMocks(null);
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
    await screen.findByRole('textbox', { name: /voice guide/i });
    fireEvent.click(screen.getByRole('button', { name: /save voice guide/i }));
    await waitFor(() => expect(mockConfirm).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith('save_project_voice_guide', expect.anything());
  });

  it('saving empty voice guide proceeds when confirmed', async () => {
    mockConfirm.mockResolvedValue(true);
    setupMocks(null);
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-abc" onClose={vi.fn()} />);
    await screen.findByRole('textbox', { name: /voice guide/i });
    fireEvent.click(screen.getByRole('button', { name: /save voice guide/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', { projectId: 'proj-abc', voiceGuide: '' }),
    );
  });
});
