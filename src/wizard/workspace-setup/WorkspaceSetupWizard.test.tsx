// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';

vi.mock('./StepFolderPick', () => ({
  default: ({ onNext }: { onNext: (path: string, repos: unknown[]) => void }) => (
    <button onClick={() => onNext('/Users/jordan/code/myorg', [
      { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
    ])}>
      next-folder
    </button>
  ),
}));
vi.mock('./StepBasicConfig', () => ({
  default: ({ onNext, onBack }: { onNext: (p: object) => void; onBack: () => void }) => (
    <div>
      <button onClick={() => onNext({
        base_url: 'https://postlane.dev', platforms: ['x'], mastodon_instance: null,
        author: 'Jordan Reyes', style: 'Direct', utm_campaign: null,
      })}>next-basic</button>
      <button onClick={onBack}>back-basic</button>
    </div>
  ),
}));
vi.mock('./StepLlm', () => ({
  default: ({ onNext, onBack }: { onNext: (p: object) => void; onBack: () => void }) => (
    <div>
      <button onClick={() => onNext({ llm_provider: 'anthropic', llm_model: 'claude-sonnet-4-6' })}>next-llm</button>
      <button onClick={onBack}>back-llm</button>
    </div>
  ),
}));
vi.mock('./StepScheduler', () => ({
  default: ({ onNext, onBack }: { onNext: (p: object) => void; onBack: () => void }) => (
    <div>
      <button onClick={() => onNext({
        scheduler_provider: 'zernio', scheduler_api_key: 'zk_secret', scheduler_profile_id: null,
      })}>next-scheduler</button>
      <button onClick={onBack}>back-scheduler</button>
    </div>
  ),
}));
vi.mock('./StepAttribution', () => ({
  default: ({ onNext, onBack }: { onNext: (p: object) => void; onBack: () => void }) => (
    <div>
      <button onClick={() => onNext({ attribution: true })}>next-attribution</button>
      <button onClick={onBack}>back-attribution</button>
    </div>
  ),
}));
vi.mock('./StepReview', () => ({
  default: ({ workspacePath, childRepos, config, onComplete, onBack }: {
    workspacePath: string; childRepos: unknown[]; config: object; onComplete: () => void; onBack: () => void;
  }) => (
    <div data-testid="review" data-workspace-path={workspacePath} data-child-repos={JSON.stringify(childRepos)} data-config={JSON.stringify(config)}>
      <button onClick={onComplete}>complete-review</button>
      <button onClick={onBack}>back-review</button>
    </div>
  ),
}));

import WorkspaceSetupWizard from './WorkspaceSetupWizard';

function renderWizard(onComplete = vi.fn(), onBack = vi.fn()) {
  render(
    <MantineProvider>
      <WorkspaceSetupWizard projectId="proj-1" projectName="North Lane" onComplete={onComplete} onBack={onBack} />
    </MantineProvider>,
  );
  return { onComplete, onBack };
}

function advanceThroughAllSteps() {
  fireEvent.click(screen.getByText('next-folder'));
  fireEvent.click(screen.getByText('next-basic'));
  fireEvent.click(screen.getByText('next-llm'));
  fireEvent.click(screen.getByText('next-scheduler'));
  fireEvent.click(screen.getByText('next-attribution'));
}

describe('WorkspaceSetupWizard — step progression', () => {
  it('starts on Step 1 (folder pick)', () => {
    renderWizard();
    expect(screen.getByText('next-folder')).toBeInTheDocument();
  });

  it('shows the project name in the header', () => {
    renderWizard();
    expect(screen.getByText('Set up "North Lane"')).toBeInTheDocument();
  });

  it('advances through all 6 steps in order', () => {
    renderWizard();
    advanceThroughAllSteps();
    expect(screen.getByTestId('review')).toBeInTheDocument();
  });

  it('going back from Step 2 returns to Step 1', () => {
    renderWizard();
    fireEvent.click(screen.getByText('next-folder'));
    fireEvent.click(screen.getByText('back-basic'));
    expect(screen.getByText('next-folder')).toBeInTheDocument();
  });
});

describe('WorkspaceSetupWizard — aggregate config accumulation', () => {
  it('threads project_id, workspacePath, childRepos, and every step patch into StepReview', () => {
    renderWizard();
    advanceThroughAllSteps();
    const review = screen.getByTestId('review');
    expect(review.dataset.workspacePath).toBe('/Users/jordan/code/myorg');
    expect(JSON.parse(review.dataset.childRepos ?? '[]')).toEqual([
      { name: 'frontend', path: '/Users/jordan/code/myorg/frontend', posts_dir: 'frontend' },
    ]);
    const config = JSON.parse(review.dataset.config ?? '{}');
    expect(config).toEqual({
      project_id: 'proj-1',
      base_url: 'https://postlane.dev',
      platforms: ['x'],
      mastodon_instance: null,
      author: 'Jordan Reyes',
      style: 'Direct',
      utm_campaign: null,
      llm_provider: 'anthropic',
      llm_model: 'claude-sonnet-4-6',
      scheduler_provider: 'zernio',
      scheduler_api_key: 'zk_secret',
      scheduler_profile_id: null,
      attribution: true,
    });
  });
});

describe('WorkspaceSetupWizard — completion', () => {
  it('calls onComplete only after StepReview signals completion, not merely reaching it', () => {
    const { onComplete } = renderWizard();
    advanceThroughAllSteps();
    expect(onComplete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('complete-review'));
    expect(onComplete).toHaveBeenCalled();
  });
});
