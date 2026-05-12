// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import ModalOrgPicker from './ModalOrgPicker';

const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);

interface OrgSummary {
  login: string;
  display_name: string;
  avatar_url: string;
  is_personal: boolean;
  has_project: boolean;
  project_id: string | null;
}

const MOCK_ORGS: OrgSummary[] = [
  { login: 'hugoelliott', display_name: 'Hugo Elliott', avatar_url: 'https://avatars.githubusercontent.com/u/1', is_personal: true, has_project: false, project_id: null },
  { login: 'neworg', display_name: 'New Org', avatar_url: 'https://avatars.githubusercontent.com/orgs/neworg', is_personal: false, has_project: false, project_id: null },
  { login: 'postlane', display_name: 'Postlane', avatar_url: 'https://avatars.githubusercontent.com/orgs/postlane', is_personal: false, has_project: true, project_id: 'existing-proj-456' },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_provider_orgs') return MOCK_ORGS;
    if (cmd === 'create_project') return { project_id: 'proj-123', name: 'Test', workspace_type: 'personal' };
    throw new Error(`Unexpected command: ${cmd}`);
  });
});

describe('ModalOrgPicker — loading and display', () => {
  it('shows loading state while list_provider_orgs is in flight', async () => {
    let resolveOrgs!: (v: OrgSummary[]) => void;
    mockInvoke.mockReturnValue(new Promise<OrgSummary[]>((r) => { resolveOrgs = r; }));
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    expect(screen.getByText(/loading/i)).toBeDefined();
    resolveOrgs([]);
  });

  it('renders org login names and avatars after loading', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
    expect(screen.getByText('postlane')).toBeDefined();
    const avatars = screen.getAllByRole('img');
    expect(avatars.length).toBeGreaterThanOrEqual(2);
    expect((avatars[0] as HTMLImageElement).src).toContain('avatars.githubusercontent.com');
  });

  it('marks personal account with Personal badge', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
    expect(screen.getByText('Personal')).toBeDefined();
  });

  it('shows Existing badge for orgs that already have a project', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('postlane')).toBeDefined());
    expect(screen.getByText('Existing')).toBeDefined();
  });

  it('does not show a cost badge for orgs without a project', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
    expect(screen.queryByText('Free')).toBeNull();
    expect(screen.queryByText('$5/month')).toBeNull();
  });

  it('next is disabled before an org is selected', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
    const nextBtn = screen.getByRole('button', { name: /next/i });
    expect((nextBtn as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('ModalOrgPicker — selection and creation', () => {
  it('selecting a new org auto-fills workspace name', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('neworg')).toBeDefined());
    await userEvent.click(screen.getByRole('option', { name: /neworg/i }));
    const input = screen.getByRole('textbox') as HTMLInputElement;
    expect(input.value).toBe('New Org');
  });

  it('user can override auto-filled name', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('neworg')).toBeDefined());
    await userEvent.click(screen.getByRole('option', { name: /neworg/i }));
    const input = screen.getByRole('textbox');
    await userEvent.clear(input);
    await userEvent.type(input, 'My Custom Name');
    expect((input as HTMLInputElement).value).toBe('My Custom Name');
  });

  it('create_project called with providerOrgLogin for a new org account', async () => {
    const onNext = vi.fn();
    render(<ModalOrgPicker onNext={onNext} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('neworg')).toBeDefined());
    await userEvent.click(screen.getByRole('option', { name: /neworg/i }));
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_project', expect.objectContaining({
        providerOrgLogin: 'neworg',
        workspaceType: 'organization',
      }));
      expect(onNext).toHaveBeenCalledWith('proj-123');
    });
  });

  it('create_project called without providerOrgLogin for personal account', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
    await userEvent.click(screen.getByRole('option', { name: /hugoelliott/i }));
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(([c]) => c === 'create_project');
      if (!call) throw new Error('create_project not called');
      expect(call[1]).not.toHaveProperty('providerOrgLogin');
      expect(call[1]).toMatchObject({ workspaceType: 'personal' });
    });
  });
});

describe('ModalOrgPicker — existing workspace (has_project: true)', () => {
  it('does not call create_project when selecting an org with an existing project', async () => {
    const onNext = vi.fn();
    render(<ModalOrgPicker onNext={onNext} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => screen.getByText('postlane'));
    await userEvent.click(screen.getByRole('option', { name: /postlane/i }));
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
    expect(mockInvoke).not.toHaveBeenCalledWith('create_project', expect.anything());
  });

  it('calls onNext with the existing project_id when org has_project is true', async () => {
    const onNext = vi.fn();
    render(<ModalOrgPicker onNext={onNext} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => screen.getByText('postlane'));
    await userEvent.click(screen.getByRole('option', { name: /postlane/i }));
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onNext).toHaveBeenCalledWith('existing-proj-456'));
  });

  it('does not show workspace name input for an org with an existing project', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => screen.getByText('postlane'));
    await userEvent.click(screen.getByRole('option', { name: /postlane/i }));
    expect(screen.queryByRole('textbox')).toBeNull();
  });

  it('next is enabled immediately after selecting an org with an existing project', async () => {
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => screen.getByText('postlane'));
    await userEvent.click(screen.getByRole('option', { name: /postlane/i }));
    const nextBtn = screen.getByRole('button', { name: /next/i });
    expect((nextBtn as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('ModalOrgPicker — errors and gates', () => {
  it('shows error state with Retry when list_provider_orgs fails', async () => {
    mockInvoke.mockRejectedValue(new Error('network timeout'));
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole('alert')).toBeDefined());
    expect(screen.getByRole('button', { name: /retry/i })).toBeDefined();
  });

  it('Retry button reloads org list after error', async () => {
    mockInvoke
      .mockRejectedValueOnce(new Error('network timeout'))
      .mockResolvedValue(MOCK_ORGS);
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /retry/i }));
    await userEvent.click(screen.getByRole('button', { name: /retry/i }));
    await waitFor(() => expect(screen.getByText('hugoelliott')).toBeDefined());
  });

  it('shows re-auth prompt on scope_not_granted error', async () => {
    mockInvoke.mockRejectedValue(new Error('scope_not_granted'));
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /sign in again/i })).toBeDefined());
  });

  it('re-auth button opens postlane.dev/login with provider', async () => {
    mockInvoke.mockRejectedValue(new Error('scope_not_granted'));
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} provider="github" />);
    await waitFor(() => screen.getByRole('button', { name: /sign in again/i }));
    await userEvent.click(screen.getByRole('button', { name: /sign in again/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith(expect.stringContaining('postlane.dev/login'));
    expect(mockOpenUrl).toHaveBeenCalledWith(expect.stringContaining('github'));
  });

  it('calls onPricingGate when create_project returns No free project slot', async () => {
    const onPricingGate = vi.fn();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_provider_orgs') return MOCK_ORGS;
      throw new Error('No free project slot. Subscribe at postlane.dev/billing');
    });
    render(<ModalOrgPicker onNext={vi.fn()} onBack={vi.fn()} onPricingGate={onPricingGate} />);
    await waitFor(() => screen.getByText('neworg'));
    await userEvent.click(screen.getByRole('option', { name: /neworg/i }));
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onPricingGate).toHaveBeenCalledOnce());
  });
});
