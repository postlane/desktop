// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import OrgLinkModal from './OrgLinkModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import { listen } from '@tauri-apps/api/event';

const mockInvoke = vi.mocked(invoke);
const mockOpenUrl = vi.mocked(openUrl);
const mockListen = vi.mocked(listen);

beforeEach(() => vi.clearAllMocks());

function setupSuccessfulConnectMocks() {
  mockInvoke.mockImplementation(async (cmd: unknown) => {
    if (cmd === 'list_provider_orgs') return ORG_LIST;
    if (cmd === 'update_project_org_login') return null;
    return null;
  });
}

const ORG_LIST = [
  { login: 'acme', display_name: 'Acme Inc', avatar_url: '', is_personal: false, has_project: false },
  { login: 'bob', display_name: 'Bob', avatar_url: '', is_personal: true, has_project: false },
];

describe('OrgLinkModal — org list', () => {
  it('renders org names once list loads', async () => {
    mockInvoke.mockResolvedValue(ORG_LIST);
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText('acme')).toBeInTheDocument());
    expect(screen.getByText('bob')).toBeInTheDocument();
  });

  it('shows loading state before orgs arrive', () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    expect(screen.getByText(/loading/i)).toBeInTheDocument();
  });

  it('shows load error message when non-scope error is returned', async () => {
    mockInvoke.mockRejectedValue(new Error('network failure'));
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText(/network failure/i)).toBeInTheDocument());
  });
});

describe('OrgLinkModal — scope error and re-auth', () => {
  it('shows scope error when scope_not_granted is returned', async () => {
    mockInvoke.mockRejectedValue(new Error('scope_not_granted'));
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /sign in again/i })).toBeInTheDocument());
  });

  it('clicking Sign in again calls openUrl with provider login URL', async () => {
    mockInvoke.mockRejectedValue(new Error('scope_not_granted'));
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} provider="github" />);
    await waitFor(() => screen.getByRole('button', { name: /sign in again/i }));
    fireEvent.click(screen.getByRole('button', { name: /sign in again/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith(expect.stringContaining('postlane.dev/login?desktop=1')));
    expect(mockOpenUrl).toHaveBeenCalledWith(expect.stringContaining('provider=github'));
  });

  it('clicking Sign in again includes port when get_local_server_port succeeds', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'list_provider_orgs') throw new Error('scope_not_granted');
      if (cmd === 'get_local_server_port') return 47312;
      return null;
    });
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} provider="github" />);
    await waitFor(() => screen.getByRole('button', { name: /sign in again/i }));
    fireEvent.click(screen.getByRole('button', { name: /sign in again/i }));
    await waitFor(() => expect(mockOpenUrl).toHaveBeenCalledWith(expect.stringContaining('port=47312')));
  });

  it('retries org list automatically after license:activated when scope error is showing', async () => {
    let fireActivated: (() => void) | undefined;
    mockListen.mockImplementation(async (event, handler) => {
      if (event === 'license:activated') fireActivated = () => handler({ event: 'license:activated', payload: {}, id: 0, windowLabel: '' });
      return () => {};
    });

    let callCount = 0;
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'list_provider_orgs') {
        callCount += 1;
        if (callCount === 1) throw new Error('scope_not_granted');
        return ORG_LIST;
      }
      if (cmd === 'get_local_server_port') return 47312;
      return null;
    });

    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /sign in again/i })).toBeInTheDocument());
    fireActivated?.();
    await waitFor(() => expect(screen.getByText('acme')).toBeInTheDocument());
  });
});

describe('OrgLinkModal — connect action', () => {
  it('calls update_project_org_login with selected org login', async () => {
    setupSuccessfulConnectMocks();
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => screen.getByText('acme'));
    fireEvent.click(screen.getByRole('option', { name: /acme/i }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('update_project_org_login', {
        projectId: 'proj-1',
        orgLogin: 'acme',
      }),
    );
  });

  it('calls onDone with org login after successful connect', async () => {
    setupSuccessfulConnectMocks();
    const onDone = vi.fn();
    render(<OrgLinkModal projectId="proj-1" onDone={onDone} onClose={() => {}} />);
    await waitFor(() => screen.getByText('acme'));
    fireEvent.click(screen.getByRole('option', { name: /acme/i }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(onDone).toHaveBeenCalledWith('acme'));
  });

  it('shows error when update_project_org_login fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'list_provider_orgs') return ORG_LIST;
      if (cmd === 'update_project_org_login') throw new Error('Server error');
      return null;
    });
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => screen.getByText('acme'));
    fireEvent.click(screen.getByRole('option', { name: /acme/i }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  });

  it('calls onClose when Cancel is clicked', async () => {
    mockInvoke.mockResolvedValue(ORG_LIST);
    const onClose = vi.fn();
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={onClose} />);
    await waitFor(() => screen.getByText('acme'));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does not create a new project — update_project_org_login is used not create_project', async () => {
    setupSuccessfulConnectMocks();
    render(<OrgLinkModal projectId="proj-1" onDone={() => {}} onClose={() => {}} />);
    await waitFor(() => screen.getByText('acme'));
    fireEvent.click(screen.getByRole('option', { name: /acme/i }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('update_project_org_login', expect.anything()));
    expect(mockInvoke).not.toHaveBeenCalledWith('create_project', expect.anything());
  });
});
