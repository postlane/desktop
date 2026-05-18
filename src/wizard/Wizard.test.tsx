// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

vi.mock('./ModalWelcome', () => ({
  default: ({ onNext }: { onNext: () => void }) => (
    <button onClick={onNext}>next-welcome</button>
  ),
}));

vi.mock('./ModalAccount', () => ({
  default: ({ onNext, onBack, mode }: { onNext: (provider: string, newLink: boolean) => void; onBack?: () => void; mode?: string }) => (
    <div>
      <span data-testid="mode">{mode}</span>
      <button onClick={() => onNext('github', false)}>next-account-github</button>
      <button onClick={() => onNext('gitlab', false)}>next-account-gitlab</button>
      {onBack && <button onClick={onBack}>back-account</button>}
    </div>
  ),
}));

vi.mock('./ModalOrgPicker', () => ({
  default: ({
    onNext,
    onBack,
    onPricingGate,
    onSkipToApp,
    provider,
  }: {
    onNext: (workspaceId: string, workspaceName: string) => void;
    onBack: () => void;
    onPricingGate: () => void;
    onSkipToApp?: () => void;
    provider?: string;
  }) => (
    <div>
      <span data-testid="org-provider">{provider}</span>
      <button onClick={() => onNext('ws-id-1', 'My Org')}>next-org</button>
      <button onClick={onBack}>back-org</button>
      <button onClick={onPricingGate}>pricing-gate</button>
      {onSkipToApp && <button onClick={onSkipToApp}>skip-org</button>}
    </div>
  ),
}));

vi.mock('./ModalScheduler', () => ({
  default: ({
    onNext,
    onBack,
    onSkipToApp,
    workspaceId,
    workspaceName,
    setSchedulerLinked,
  }: {
    onNext: () => void;
    onBack: () => void;
    onSkipToApp?: () => void;
    workspaceId: string;
    workspaceName: string;
    setSchedulerLinked: (linked: boolean) => void;
  }) => (
    <div>
      <span data-testid="scheduler-workspace-id">{workspaceId}</span>
      <span data-testid="scheduler-workspace-name">{workspaceName}</span>
      <button onClick={onNext}>next-scheduler</button>
      <button onClick={onBack}>back-scheduler</button>
      <button onClick={() => setSchedulerLinked(true)}>set-linked</button>
      {onSkipToApp && <button onClick={onSkipToApp}>skip-scheduler</button>}
    </div>
  ),
}));

vi.mock('./ModalGitHubApp', () => ({
  default: ({
    onNext,
    onBack,
    provider,
    workspaceId,
    workspaceName,
  }: {
    onNext: () => void;
    onBack: () => void;
    provider: string;
    workspaceId: string;
    workspaceName: string;
  }) => (
    <div>
      <span data-testid="github-app-provider">{provider}</span>
      <span data-testid="github-app-workspace-id">{workspaceId}</span>
      <span data-testid="github-app-workspace-name">{workspaceName}</span>
      <button onClick={onNext}>next-github-app</button>
      <button onClick={onBack}>back-github-app</button>
    </div>
  ),
}));

vi.mock('./ModalProjectContext', () => ({
  default: ({
    onNext,
    onBack,
    workspaceId,
    workspaceName,
  }: {
    onNext: () => void;
    onBack: () => void;
    workspaceId: string;
    workspaceName: string;
  }) => (
    <div>
      <span data-testid="project-context-workspace-id">{workspaceId}</span>
      <span data-testid="project-context-workspace-name">{workspaceName}</span>
      <button onClick={onNext}>next-project-context</button>
      <button onClick={onBack}>back-project-context</button>
    </div>
  ),
}));

vi.mock('./ModalComplete', () => ({
  default: ({
    onComplete,
    onBack,
    schedulerLinked,
  }: {
    onComplete: () => void;
    onBack: () => void;
    schedulerLinked: boolean;
  }) => (
    <div>
      <span data-testid="complete-scheduler-linked">{String(schedulerLinked)}</span>
      <button onClick={onComplete}>next-complete</button>
      <button onClick={onBack}>back-complete</button>
    </div>
  ),
}));

vi.mock('./ModalProviderLinked', () => ({ default: () => null }));

vi.mock('./ModalPricingGate', () => ({
  default: ({
    onPaid,
    onBack,
    onSkip,
  }: {
    onPaid: () => void;
    onBack: () => void;
    onSkip?: (projectId: string, projectName: string) => void;
  }) => (
    <div>
      <button onClick={onPaid}>paid</button>
      <button onClick={onBack}>back-pricing</button>
      {onSkip && <button onClick={() => onSkip('proj-1', 'Proj Name')}>skip-pricing</button>}
    </div>
  ),
}));

import { invoke } from '../ipc/invoke';
import Wizard from './Wizard';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(undefined);
});

describe('Wizard — steps 1 and 2', () => {
  it('test_renders_step_1_welcome', () => {
    render(<Wizard onComplete={vi.fn()} />);
    expect(screen.getByText('next-welcome')).toBeDefined();
  });

  it('test_step_1_next_advances_to_step_2', () => {
    render(<Wizard onComplete={vi.fn()} />);
    fireEvent.click(screen.getByText('next-welcome'));
    expect(screen.getByText('next-account-github')).toBeDefined();
  });

  it('test_renders_step_2_sign_in_mode_when_start_at_default', () => {
    render(<Wizard onComplete={vi.fn()} />);
    fireEvent.click(screen.getByText('next-welcome'));
    expect(screen.getByTestId('mode').textContent).toBe('sign_in');
  });

  it('test_renders_step_2_add_org_mode_when_start_at_2', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    expect(screen.getByTestId('mode').textContent).toBe('add_org');
  });

  it('test_step_2_next_with_github_advances_to_step_3', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-github'));
    expect(screen.getByTestId('org-provider').textContent).toBe('github');
  });

  it('test_step_2_next_with_gitlab_advances_to_step_3_with_gitlab_provider', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-gitlab'));
    expect(screen.getByTestId('org-provider').textContent).toBe('gitlab');
  });

  it('test_step_2_sets_token_and_provider_on_next', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-github'));
    expect(screen.getByTestId('org-provider').textContent).toBe('github');
  });
});

describe('Wizard — step 3', () => {
  it('test_renders_step_3_org_picker', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    expect(screen.getByText('next-org')).toBeDefined();
  });

  it('test_step_3_next_advances_to_step_4_with_workspace_data', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('next-org'));
    expect(screen.getByTestId('scheduler-workspace-id').textContent).toBe('ws-id-1');
    expect(screen.getByTestId('scheduler-workspace-name').textContent).toBe('My Org');
  });

  it('test_step_3_pricing_gate_shows_pricing_modal', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('pricing-gate'));
    expect(screen.getByText('paid')).toBeDefined();
    expect(screen.getByText('back-pricing')).toBeDefined();
  });

  it('test_pricing_gate_back_returns_to_org_picker', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('pricing-gate'));
    fireEvent.click(screen.getByText('back-pricing'));
    expect(screen.getByText('next-org')).toBeDefined();
  });

  it('test_pricing_gate_paid_closes_gate_and_returns_to_org_picker', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('pricing-gate'));
    fireEvent.click(screen.getByText('paid'));
    expect(screen.getByText('next-org')).toBeDefined();
  });

  it('test_pricing_gate_skip_sets_workspace_and_advances_to_step_4', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('pricing-gate'));
    fireEvent.click(screen.getByText('skip-pricing'));
    expect(screen.getByTestId('scheduler-workspace-id').textContent).toBe('proj-1');
    expect(screen.getByTestId('scheduler-workspace-name').textContent).toBe('Proj Name');
  });
});

describe('Wizard — step 4', () => {
  it('test_renders_step_4_scheduler', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    expect(screen.getByText('next-scheduler')).toBeDefined();
  });

  it('test_step_4_next_advances_to_step_5', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    fireEvent.click(screen.getByText('next-scheduler'));
    expect(screen.getByText('next-github-app')).toBeDefined();
  });

  it('test_step_4_skip_advances_to_step_5_not_complete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={4} />);
    fireEvent.click(screen.getByText('skip-scheduler'));
    expect(screen.getByText('next-github-app')).toBeDefined();
    expect(onComplete).not.toHaveBeenCalled();
  });
});

describe('Wizard — step 5 and completion', () => {
  it('test_renders_step_5_github_app', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByText('next-github-app')).toBeDefined();
  });

  it('test_step_5_next_advances_to_step_6_voice_guide', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    fireEvent.click(screen.getByText('next-github-app'));
    expect(screen.getByText('next-project-context')).toBeDefined();
  });

  it('test_step_6_next_advances_to_step_7_complete', () => {
    render(<Wizard onComplete={vi.fn()} startAt={6} />);
    fireEvent.click(screen.getByText('next-project-context'));
    expect(screen.getByText('next-complete')).toBeDefined();
  });

  it('test_step_7_complete_calls_on_complete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={7} />);
    fireEvent.click(screen.getByText('next-complete'));
    await waitFor(() => expect(onComplete).toHaveBeenCalled());
  });

  it('test_step_6_workspace_id_passed_to_project_context', () => {
    render(<Wizard onComplete={vi.fn()} startAt={6} />);
    expect(screen.getByTestId('project-context-workspace-id').textContent).toBe('');
  });

  it('test_step_3_skip_to_app_calls_set_wizard_completed_and_on_complete', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={3} />);
    fireEvent.click(screen.getByText('skip-org'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('set_wizard_completed'));
    expect(onComplete).toHaveBeenCalled();
  });

  it('test_handle_skip_to_app_calls_on_complete_even_when_invoke_throws', async () => {
    mockInvoke.mockRejectedValue(new Error('ipc error'));
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={3} />);
    fireEvent.click(screen.getByText('skip-org'));
    await waitFor(() => expect(onComplete).toHaveBeenCalled());
  });

  it('test_step_5_default_provider_is_github_when_not_set', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    expect(screen.getByTestId('github-app-provider').textContent).toBe('github');
  });

  it('test_step_4_workspace_data_passed_through_from_step_3', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('next-org'));
    expect(screen.getByTestId('scheduler-workspace-id').textContent).toBe('ws-id-1');
    expect(screen.getByTestId('scheduler-workspace-name').textContent).toBe('My Org');
  });
});

describe('Wizard — wizard state persistence', () => {
  it('write_wizard_state includes workspaceId and workspaceName after step 3 next', async () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('next-org'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('write_wizard_state', {
        step: 4,
        workspaceId: 'ws-id-1',
        workspaceName: 'My Org',
      })
    );
  });

  it('initialWorkspaceId seeds workspaceId in step 6 project context', () => {
    render(<Wizard onComplete={vi.fn()} startAt={6} initialWorkspaceId="ws-saved" initialWorkspaceName="Saved Org" />);
    expect(screen.getByTestId('project-context-workspace-id').textContent).toBe('ws-saved');
    expect(screen.getByTestId('project-context-workspace-name').textContent).toBe('Saved Org');
  });

  it('initialWorkspaceId seeds workspaceId in step 4 scheduler', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} initialWorkspaceId="ws-saved" initialWorkspaceName="Saved Org" />);
    expect(screen.getByTestId('scheduler-workspace-id').textContent).toBe('ws-saved');
    expect(screen.getByTestId('scheduler-workspace-name').textContent).toBe('Saved Org');
  });
});

describe('Wizard — back navigation and full flow', () => {
  it('test_back_navigation_from_step_2_returns_to_step_1', () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('back-account'));
    expect(screen.getByText('next-welcome')).toBeDefined();
  });

  it('test_back_navigation_from_step_3_returns_to_step_2', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('back-org'));
    expect(screen.getByText('next-account-github')).toBeDefined();
  });

  it('test_back_navigation_from_step_4_returns_to_step_3', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    fireEvent.click(screen.getByText('back-scheduler'));
    expect(screen.getByText('next-org')).toBeDefined();
  });

  it('test_back_navigation_from_step_5_returns_to_step_4', () => {
    render(<Wizard onComplete={vi.fn()} startAt={5} />);
    fireEvent.click(screen.getByText('back-github-app'));
    expect(screen.getByText('next-scheduler')).toBeDefined();
  });

  it('test_full_wizard_flow_from_step_1_to_completion', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('next-welcome'));
    fireEvent.click(screen.getByText('next-account-github'));
    fireEvent.click(screen.getByText('next-org'));
    fireEvent.click(screen.getByText('next-scheduler'));
    fireEvent.click(screen.getByText('next-github-app'));
    fireEvent.click(screen.getByText('next-project-context'));
    fireEvent.click(screen.getByText('next-complete'));
    await waitFor(() => expect(onComplete).toHaveBeenCalled());
  });
});
