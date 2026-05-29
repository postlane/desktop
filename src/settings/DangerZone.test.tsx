// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.6 DangerZone component.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import DangerZone from './DangerZone';

// ── invoke mock ───────────────────────────────────────────────────────────────

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...args: unknown[]) => mockInvoke(...args) }));

function defaultInfo() {
  return { workspace_path: '/home/user/code/myorg', name: 'myorg' };
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_workspace_info') return Promise.resolve(defaultInfo());
    if (cmd === 'check_workspace_journal') return Promise.resolve(false);
    if (cmd === 'disconnect_workspace') return Promise.resolve(true);
    if (cmd === 'delete_workspace') return Promise.resolve(false);
    return Promise.resolve(null);
  });
});

// ── 22.6.1: visible only to owner ────────────────────────────────────────────

describe('22.6.1: owner-only visibility', () => {
  it('renders nothing when isOwner is false', () => {
    const { container } = render(<DangerZone workspaceId="ws-1" isOwner={false} />);
    expect(container.firstChild).toBeNull();
  });

  it('renders the danger zone when isOwner is true', async () => {
    render(<DangerZone workspaceId="ws-1" isOwner />);
    await waitFor(() => expect(screen.getByText(/Danger Zone/i)).toBeDefined());
  });
});

// ── 22.6.1: collapsed by default ─────────────────────────────────────────────

describe('22.6.1: collapsed by default', () => {
  it('does not show buttons when collapsed', async () => {
    render(<DangerZone workspaceId="ws-1" isOwner />);
    await act(async () => {});
    expect(screen.queryByText(/Disconnect this workspace/i)).toBeNull();
    expect(screen.queryByText(/Delete workspace/i)).toBeNull();
  });

  it('shows buttons after expanding', async () => {
    render(<DangerZone workspaceId="ws-1" isOwner />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => expect(screen.getByText(/Disconnect this workspace/i)).toBeDefined());
    expect(screen.getByText(/Delete workspace/i)).toBeDefined();
  });
});

// ── 22.6.2: Disconnect confirmation ──────────────────────────────────────────

describe('22.6.2: Disconnect confirmation dialog', () => {
  async function openDisconnect() {
    render(<DangerZone workspaceId="ws-1" isOwner />);
    await act(async () => {}); // flush useWorkspaceInfo async effect
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => screen.getByRole('button', { name: /Disconnect this workspace/i }));
    fireEvent.click(screen.getByRole('button', { name: /Disconnect this workspace/i }));
    await waitFor(() => screen.getByRole('dialog', { name: /Disconnect workspace/i }));
  }

  it('shows confirmation dialog after clicking Disconnect', async () => {
    await openDisconnect();
    expect(screen.queryByText(/leaves your files intact/i)).not.toBeNull();
  });

  it('shows workspace name in confirmation', async () => {
    await openDisconnect();
    await waitFor(() => expect(screen.getByText(/myorg/)).toBeDefined());
  });

  it('Cancel closes dialog without calling disconnect_workspace', async () => {
    await openDisconnect();
    fireEvent.click(screen.getByText(/Cancel/i));
    expect(mockInvoke).not.toHaveBeenCalledWith('disconnect_workspace', expect.anything());
    expect(screen.queryByText(/leaves your files intact/i)).toBeNull();
  });

  it('Disconnect button calls disconnect_workspace with correct workspaceId', async () => {
    await openDisconnect();
    fireEvent.click(screen.getByRole('button', { name: /^Disconnect$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_workspace', { workspaceId: 'ws-1' })
    );
  });
});

// ── 22.6.10/22.6.11: Delete two-step confirmation ────────────────────────────

describe('22.6.10/22.6.11: Delete two-step confirmation', () => {
  async function openDeleteStep1() {
    render(<DangerZone workspaceId="ws-1" isOwner />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => screen.getByRole('button', { name: /Delete workspace/i }));
    fireEvent.click(screen.getByRole('button', { name: /Delete workspace/i }));
    await act(async () => {});
  }

  it('shows Step 1 warning on clicking Delete', async () => {
    await openDeleteStep1();
    expect(screen.getByText(/cannot be undone/i)).toBeDefined();
  });

  it('shows Continue button on Step 1', async () => {
    await openDeleteStep1();
    expect(screen.getByRole('button', { name: /Continue/i })).toBeDefined();
  });

  it('Cancel on Step 1 closes without invoking delete', async () => {
    await openDeleteStep1();
    const cancelBtn = screen.getAllByText(/Cancel/i)[0];
    fireEvent.click(cancelBtn);
    expect(mockInvoke).not.toHaveBeenCalledWith('delete_workspace', expect.anything());
    expect(screen.queryByText(/cannot be undone/i)).toBeNull();
  });

  it('Continue advances to Step 2 with name input', async () => {
    await openDeleteStep1();
    fireEvent.click(screen.getByRole('button', { name: /Continue/i }));
    await waitFor(() => expect(screen.getByLabelText(/type the workspace name/i)).toBeDefined());
  });
});

// ── Shared step-2 helper ─────────────────────────────────────────────────────

async function openDeleteStep2() {
  render(<DangerZone workspaceId="ws-1" isOwner />);
  await act(async () => {});
  fireEvent.click(screen.getByText(/Danger Zone/i));
  await waitFor(() => screen.getByRole('button', { name: /Delete workspace/i }));
  fireEvent.click(screen.getByRole('button', { name: /Delete workspace/i }));
  await act(async () => {});
  await waitFor(() => screen.getByRole('button', { name: /Continue/i }));
  fireEvent.click(screen.getByRole('button', { name: /Continue/i }));
  await act(async () => {});
  await waitFor(() => screen.getByLabelText(/type the workspace name/i));
}

// ── 22.6.11: Step 2 — name confirmation ──────────────────────────────────────

describe('22.6.11/22.6.19: Step 2 name confirmation', () => {

  it('Delete button disabled when input is empty', async () => {
    await openDeleteStep2();
    const btn = screen.getByRole('button', { name: /^Delete$/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it('Delete button disabled when input does not match basename', async () => {
    await openDeleteStep2();
    fireEvent.change(screen.getByLabelText(/type the workspace name/i), {
      target: { value: 'wrongname' },
    });
    const btn = screen.getByRole('button', { name: /^Delete$/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it('Delete button enabled when input exactly matches basename', async () => {
    await openDeleteStep2();
    fireEvent.change(screen.getByLabelText(/type the workspace name/i), {
      target: { value: 'myorg' },
    });
    const btn = screen.getByRole('button', { name: /^Delete$/i });
    expect((btn as HTMLButtonElement).disabled).toBe(false);
  });

  it('Delete button re-disabled when input is changed away from match', async () => {
    await openDeleteStep2();
    const input = screen.getByLabelText(/type the workspace name/i);
    fireEvent.change(input, { target: { value: 'myorg' } });
    fireEvent.change(input, { target: { value: 'myorg-' } });
    const btn = screen.getByRole('button', { name: /^Delete$/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it('full path shown in a read-only field, not in the input', async () => {
    await openDeleteStep2();
    const pathDisplay = screen.getByText('/home/user/code/myorg');
    expect(pathDisplay).toBeDefined();
    const input = screen.getByLabelText(/type the workspace name/i) as HTMLInputElement;
    expect(input.value).toBe('');
    expect(input.readOnly).toBe(false);
  });

  it('Delete confirms by calling delete_workspace', async () => {
    await openDeleteStep2();
    fireEvent.change(screen.getByLabelText(/type the workspace name/i), {
      target: { value: 'myorg' },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Delete$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('delete_workspace', { workspaceId: 'ws-1' })
    );
  });
});

// ── 22.6.12a: Migration journal warning ──────────────────────────────────────

async function openDeleteWithJournal() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_workspace_info') return Promise.resolve(defaultInfo());
    if (cmd === 'check_workspace_journal') return Promise.resolve(true);
    if (cmd === 'delete_workspace') return Promise.resolve(false);
    return Promise.resolve(null);
  });
  render(<DangerZone workspaceId="ws-1" isOwner />);
  await act(async () => {});
  fireEvent.click(screen.getByText(/Danger Zone/i));
  await waitFor(() => screen.getByRole('button', { name: /Delete workspace/i }));
  fireEvent.click(screen.getByRole('button', { name: /Delete workspace/i }));
  await act(async () => {});
  await waitFor(() => screen.getByRole('button', { name: /Continue/i }));
  fireEvent.click(screen.getByRole('button', { name: /Continue/i }));
  await act(async () => {});
}

describe('22.6.12a: journal warning before Step 2', () => {

  it('shows journal warning before Step 2 when journal exists', async () => {
    await openDeleteWithJournal();
    await waitFor(() =>
      expect(screen.getByText(/migration is in progress/i)).toBeDefined()
    );
  });

  it('requires acknowledgement before reaching Step 2', async () => {
    await openDeleteWithJournal();
    await waitFor(() => screen.getByText(/migration is in progress/i));
    expect(screen.queryByLabelText(/type the workspace name/i)).toBeNull();
    expect(screen.getByRole('button', { name: /I understand/i })).toBeDefined();
  });

  it('clicking I understand shows Step 2', async () => {
    await openDeleteWithJournal();
    await waitFor(() => screen.getByText(/migration is in progress/i));
    fireEvent.click(screen.getByRole('button', { name: /I understand/i }));
    await waitFor(() => expect(screen.getByLabelText(/type the workspace name/i)).toBeDefined());
  });
});
