// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoConfigureModal from './RepoConfigureModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ confirm: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('RepoConfigureModal — Escape key closes modal', () => {
  it('calls onClose when Escape is pressed', async () => {
    const onClose = vi.fn();
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={onClose} />);
    await waitFor(() => screen.getByText(/using default credentials/i));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not call onClose for other keys', async () => {
    const onClose = vi.fn();
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={onClose} />);
    await waitFor(() => screen.getByText(/using default credentials/i));
    fireEvent.keyDown(document, { key: 'Enter' });
    expect(onClose).not.toHaveBeenCalled();
  });
});

describe('RepoConfigureModal — test error non-Error fallback', () => {
  it('shows "Test failed" when test_scheduler rejects with a non-Error value', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'test_scheduler') throw 'raw string rejection';
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const testBtn = await screen.findByRole('button', { name: /test connection/i });
    fireEvent.click(testBtn);
    await waitFor(() =>
      expect(screen.getByText(/test failed/i)).toBeInTheDocument(),
    );
  });
});

describe('RepoConfigureModal — remove non-Error fallback', () => {
  it('shows "Failed to remove credential" when remove throws a non-Error', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') throw 'unexpected string error';
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(screen.getByText(/failed to remove credential/i)).toBeInTheDocument(),
    );
  });
});

describe('RepoConfigureModal — projectId renders VoiceGuideSection', () => {
  it('renders VoiceGuideSection when projectId is provided', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'get_project_voice_guide') return null;
      return null;
    });
    render(
      <RepoConfigureModal
        repoId="r1" repoName="my-repo" currentProvider="zernio"
        projectId="proj-123" onClose={vi.fn()}
      />
    );
    await waitFor(() => screen.getByText(/using default credentials/i));
    expect(screen.getByText(/voice guide/i)).toBeInTheDocument();
  });

  it('does not render VoiceGuideSection when projectId is not provided', async () => {
    mockInvoke.mockResolvedValue(null);
    render(
      <RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />
    );
    await waitFor(() => screen.getByText(/using default credentials/i));
    expect(screen.queryByLabelText(/voice guide/i)).not.toBeInTheDocument();
  });
});

describe('RepoConfigureModal — handleSave empty key guard', () => {
  it('save button is disabled when key input is empty', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    await screen.findByPlaceholderText(/api key/i);
    expect(screen.getByRole('button', { name: /^save$/i })).toBeDisabled();
  });
});

describe('RepoConfigureModal — save non-Error fallback', () => {
  it('shows "Failed to save" when save_repo_scheduler_key rejects with a non-Error', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'save_repo_scheduler_key') throw 'string save error';
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const keyInput = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(keyInput, { target: { value: 'some-key' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(screen.getByText(/failed to save/i)).toBeInTheDocument(),
    );
  });
});
