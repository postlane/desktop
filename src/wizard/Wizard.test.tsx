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

vi.mock('./workspace-setup/WorkspaceSetupWizard', () => ({
  default: ({
    projectId,
    projectName,
    onComplete,
    onBack,
  }: {
    projectId: string;
    projectName: string;
    onComplete: () => void;
    onBack: () => void;
  }) => (
    <div>
      <span data-testid="setup-wizard-project-id">{projectId}</span>
      <span data-testid="setup-wizard-project-name">{projectName}</span>
      <button onClick={onComplete}>complete-setup-wizard</button>
      <button onClick={onBack}>back-setup-wizard</button>
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

  it('test_step_3_next_advances_to_workspace_setup_wizard_with_workspace_data', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('next-org'));
    expect(screen.getByTestId('setup-wizard-project-id').textContent).toBe('ws-id-1');
    expect(screen.getByTestId('setup-wizard-project-name').textContent).toBe('My Org');
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

  it('test_pricing_gate_skip_sets_workspace_and_advances_to_workspace_setup_wizard', () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('pricing-gate'));
    fireEvent.click(screen.getByText('skip-pricing'));
    expect(screen.getByTestId('setup-wizard-project-id').textContent).toBe('proj-1');
    expect(screen.getByTestId('setup-wizard-project-name').textContent).toBe('Proj Name');
  });
});

describe('Wizard — WorkspaceSetupWizard handoff (checklist 24.3.7)', () => {
  it('renders WorkspaceSetupWizard once step reaches 4', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    expect(screen.getByText('complete-setup-wizard')).toBeDefined();
  });

  it('passes initialWorkspaceId/initialWorkspaceName through as projectId/projectName', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} initialWorkspaceId="ws-saved" initialWorkspaceName="Saved Org" />);
    expect(screen.getByTestId('setup-wizard-project-id').textContent).toBe('ws-saved');
    expect(screen.getByTestId('setup-wizard-project-name').textContent).toBe('Saved Org');
  });

  it('calls onComplete when WorkspaceSetupWizard signals completion', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} startAt={4} />);
    fireEvent.click(screen.getByText('complete-setup-wizard'));
    await waitFor(() => expect(onComplete).toHaveBeenCalled());
  });

  it('going back from WorkspaceSetupWizard returns to the org picker (step 3)', () => {
    render(<Wizard onComplete={vi.fn()} startAt={4} />);
    fireEvent.click(screen.getByText('back-setup-wizard'));
    expect(screen.getByText('next-org')).toBeDefined();
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
});

describe('Wizard — wizard state persistence', () => {
  it('write_wizard_state includes workspaceId and workspaceName after step 3 next', async () => {
    render(<Wizard onComplete={vi.fn()} startAt={3} />);
    fireEvent.click(screen.getByText('next-org'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('write_wizard_state', expect.objectContaining({
        step: 4,
        workspaceId: 'ws-id-1',
        workspaceName: 'My Org',
      }))
    );
  });

  it('write_wizard_state includes provider after step 2 next', async () => {
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-github'));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('write_wizard_state', expect.objectContaining({
        step: 3,
        provider: 'github',
      }))
    );
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

  it('test_full_wizard_flow_from_step_1_to_completion', async () => {
    const onComplete = vi.fn();
    render(<Wizard onComplete={onComplete} />);
    fireEvent.click(screen.getByText('next-welcome'));
    fireEvent.click(screen.getByText('next-account-github'));
    fireEvent.click(screen.getByText('next-org'));
    fireEvent.click(screen.getByText('complete-setup-wizard'));
    await waitFor(() => expect(onComplete).toHaveBeenCalled());
  });
});
