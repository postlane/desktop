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
  default: ({ onNext }: { onNext: (provider: string, newLink: boolean) => void }) => (
    <div>
      <button onClick={() => onNext('github', false)}>next-account-github</button>
      <button onClick={() => onNext('gitlab', true)}>next-account-gitlab</button>
    </div>
  ),
}));

vi.mock('./ModalOrgPicker', () => ({
  default: ({
    onNext,
    onBack,
    onPricingGate,
    provider,
  }: {
    onNext: (workspaceId: string, workspaceName: string) => void;
    onBack: () => void;
    onPricingGate: () => void;
    provider?: string;
  }) => (
    <div>
      <span data-testid="org-provider">{provider}</span>
      <button onClick={() => onNext('ws-id-1', 'My Org')}>next-org</button>
      <button onClick={onBack}>back-org</button>
      <button onClick={onPricingGate}>pricing-gate</button>
    </div>
  ),
}));

vi.mock('./ModalScheduler', () => ({ default: () => <div>scheduler</div> }));
vi.mock('./ModalGitHubApp', () => ({ default: () => <div>github-app</div> }));
vi.mock('./ModalProjectContext', () => ({ default: () => <div>project-context</div> }));
vi.mock('./ModalComplete', () => ({ default: () => <div>complete</div> }));
vi.mock('./ModalPricingGate', () => ({ default: () => <div>pricing-gate-modal</div> }));

vi.mock('./ModalProviderLinked', () => ({
  default: ({
    currentProvider,
    linkedProviders,
    onContinue,
  }: {
    currentProvider: string;
    linkedProviders: string[];
    onContinue: () => void;
  }) => (
    <div>
      <span data-testid="provider-linked-current">{currentProvider}</span>
      <span data-testid="provider-linked-list">{linkedProviders.join(',')}</span>
      <button onClick={onContinue}>continue-provider-linked</button>
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

describe('Wizard — provider linking confirmation', () => {
  it('test_shows_provider_linked_screen_when_multiple_providers_returned', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_linked_providers') return ['github', 'gitlab'];
      return undefined;
    });
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-gitlab'));
    await waitFor(() => expect(screen.getByTestId('provider-linked-current')).toBeDefined());
    expect(screen.getByTestId('provider-linked-current').textContent).toBe('gitlab');
    expect(screen.getByTestId('provider-linked-list').textContent).toBe('github,gitlab');
  });

  it('test_provider_linked_continue_shows_org_picker', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_linked_providers') return ['github', 'gitlab'];
      return undefined;
    });
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-gitlab'));
    await waitFor(() => screen.getByText('continue-provider-linked'));
    fireEvent.click(screen.getByText('continue-provider-linked'));
    expect(screen.getByTestId('org-provider')).toBeDefined();
  });

  it('test_no_provider_linked_screen_when_only_one_provider', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_linked_providers') return ['github'];
      return undefined;
    });
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-github'));
    await waitFor(() => expect(screen.getByTestId('org-provider')).toBeDefined());
    expect(screen.queryByTestId('provider-linked-current')).toBeNull();
  });

  it('test_does_not_show_provider_linked_screen_when_sign_in_is_returning_user', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_linked_providers') return ['github', 'gitlab'];
      return undefined;
    });
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-github')); // newLink=false
    await waitFor(() => expect(screen.getByTestId('org-provider')).toBeDefined());
    expect(screen.queryByTestId('provider-linked-current')).toBeNull();
  });

  it('test_provider_linked_screen_not_shown_twice_on_back_and_forward', async () => {
    let linkedCheckCount = 0;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_linked_providers') { linkedCheckCount++; return ['github', 'gitlab']; }
      return undefined;
    });
    render(<Wizard onComplete={vi.fn()} startAt={2} />);
    fireEvent.click(screen.getByText('next-account-gitlab'));
    await waitFor(() => screen.getByText('continue-provider-linked'));
    fireEvent.click(screen.getByText('continue-provider-linked'));
    fireEvent.click(screen.getByText('back-org'));
    fireEvent.click(screen.getByText('next-account-gitlab'));
    await waitFor(() => expect(screen.getByTestId('org-provider')).toBeDefined());
    expect(screen.queryByTestId('provider-linked-current')).toBeNull();
    expect(linkedCheckCount).toBe(1);
  });
});
