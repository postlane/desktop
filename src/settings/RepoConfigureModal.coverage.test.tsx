// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoConfigureModal from './RepoConfigureModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('./VoiceGuideSection', () => ({ VoiceGuideSection: () => <div data-testid="voice-guide-section">Voice Guide</div> }));

beforeEach(() => vi.clearAllMocks());

describe('RepoConfigureModal — dialog structure', () => {
  it('renders with dialog role and aria-modal', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });

  it('renders VoiceGuideSection when projectId is provided', () => {
    render(<RepoConfigureModal repoName="my-repo" projectId="proj-123" onClose={vi.fn()} />);
    expect(screen.getByTestId('voice-guide-section')).toBeInTheDocument();
  });

  it('does not render VoiceGuideSection when projectId is not provided', () => {
    render(<RepoConfigureModal repoName="my-repo" onClose={vi.fn()} />);
    expect(screen.queryByTestId('voice-guide-section')).not.toBeInTheDocument();
  });
});
