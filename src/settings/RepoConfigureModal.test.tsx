// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoConfigureModal from './RepoConfigureModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('./VoiceGuideSection', () => ({ VoiceGuideSection: () => <div>Voice Guide</div> }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('RepoConfigureModal — modal shell', () => {
  it('renders with repo name in title', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.getByText('Configure my-repo')).toBeInTheDocument();
  });

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn();
    render(<RepoConfigureModal repoName="my-repo" onClose={onClose} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not call onClose for non-Escape keys', () => {
    const onClose = vi.fn();
    render(<RepoConfigureModal repoName="my-repo" onClose={onClose} />);
    fireEvent.keyDown(document, { key: 'Enter' });
    expect(onClose).not.toHaveBeenCalled();
  });

  it('calls onClose when the footer Close button is clicked', () => {
    const onClose = vi.fn();
    render(<RepoConfigureModal repoName="my-repo" onClose={onClose} />);
    // getByText targets text content — distinguishes footer "Close" from header × (no text content)
    fireEvent.click(screen.getByText('Close'));
    expect(onClose).toHaveBeenCalledOnce();
  });
});

describe('RepoConfigureModal — no per-repo scheduler', () => {
  it('does not call any scheduler IPC commands on mount', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(mockInvoke).not.toHaveBeenCalledWith('get_per_repo_scheduler_key', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('save_repo_scheduler_key', expect.anything());
    expect(mockInvoke).not.toHaveBeenCalledWith('remove_repo_scheduler_key', expect.anything());
  });

  it('does not render scheduler status text', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.queryByText(/using default credentials/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/using separate account/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/no scheduler configured/i)).not.toBeInTheDocument();
  });

  it('does not render per-repo scheduler action buttons', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /use a different account/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /close and open default scheduler/i })).not.toBeInTheDocument();
  });
});

describe('RepoConfigureModal — voice guide section', () => {
  it('renders VoiceGuideSection when projectId is provided', () => {
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-123" onClose={vi.fn()} />);
    expect(screen.getByText('Voice Guide')).toBeInTheDocument();
  });

  it('does not render VoiceGuideSection when projectId is absent', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.queryByText('Voice Guide')).not.toBeInTheDocument();
  });
});
