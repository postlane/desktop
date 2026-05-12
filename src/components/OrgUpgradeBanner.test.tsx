// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import OrgUpgradeBanner from './OrgUpgradeBanner';
import type { Project } from '../types';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'proj-1',
    name: 'Acme',
    workspace_type: 'organization',
    tier: 'free',
    billing_active: true,
    is_owner: true,
    provider_org_login: null,
    ...overrides,
  };
}

function stubAppState(dismissed: boolean) {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'read_app_state_command') {
      return { org_upgrade_banner_dismissed_v1_2: dismissed };
    }
    return null;
  });
}

describe('OrgUpgradeBanner — visibility', () => {
  it('renders the banner when org login is missing and not dismissed', async () => {
    stubAppState(false);
    render(<OrgUpgradeBanner project={makeProject()} onConnect={() => {}} />);
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  });

  it('does not render when project already has provider_org_login', async () => {
    stubAppState(false);
    render(<OrgUpgradeBanner project={makeProject({ provider_org_login: 'my-org' })} onConnect={() => {}} />);
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });

  it('does not render when user is not the owner', async () => {
    stubAppState(false);
    render(<OrgUpgradeBanner project={makeProject({ is_owner: false })} onConnect={() => {}} />);
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });

  it('does not render when banner has been dismissed', async () => {
    stubAppState(true);
    render(<OrgUpgradeBanner project={makeProject()} onConnect={() => {}} />);
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });
});

describe('OrgUpgradeBanner — interactions', () => {
  it('calls onConnect when Connect button is clicked', async () => {
    stubAppState(false);
    const onConnect = vi.fn();
    render(<OrgUpgradeBanner project={makeProject()} onConnect={onConnect} />);
    await waitFor(() => screen.getByRole('alert'));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    expect(onConnect).toHaveBeenCalledTimes(1);
  });

  it('hides banner after Dismiss is clicked', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'read_app_state_command') return { org_upgrade_banner_dismissed_v1_2: false };
      if (cmd === 'save_app_state_command') return null;
      return null;
    });
    render(<OrgUpgradeBanner project={makeProject()} onConnect={() => {}} />);
    await waitFor(() => screen.getByRole('alert'));
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  });

  it('persists dismissal via save_app_state_command', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'read_app_state_command') return { org_upgrade_banner_dismissed_v1_2: false };
      if (cmd === 'save_app_state_command') return null;
      return null;
    });
    render(<OrgUpgradeBanner project={makeProject()} onConnect={() => {}} />);
    await waitFor(() => screen.getByRole('alert'));
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_app_state_command',
        expect.objectContaining({
          state: expect.objectContaining({ org_upgrade_banner_dismissed_v1_2: true }),
        }),
      ),
    );
  });
});
