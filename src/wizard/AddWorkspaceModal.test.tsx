// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AddWorkspaceModal from './AddWorkspaceModal';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('AddWorkspaceModal', () => {
  it('renders a name input and a workspace type selector', () => {
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={vi.fn()} />);
    expect(screen.getByRole('textbox', { name: /workspace name/i })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: /workspace type/i })).toBeInTheDocument();
  });

  it('shows an inline validation error and does not call create_project when name is empty', async () => {
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    expect(await screen.findByText(/name is required/i)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('calls create_project with the entered name and workspace type on submit', async () => {
    mockInvoke.mockResolvedValue({ project_id: 'p-1', name: 'My Org', workspace_type: 'organization' });
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'My Org' } });
    fireEvent.change(screen.getByRole('combobox', { name: /workspace type/i }), { target: { value: 'organization' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'My Org', workspaceType: 'organization' }));
  });

  it('calls onCreated after a successful create', async () => {
    const onCreated = vi.fn();
    mockInvoke.mockResolvedValue({ project_id: 'p-1', name: 'My Org', workspace_type: 'personal' });
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={onCreated} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'My Org' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    await waitFor(() => expect(onCreated).toHaveBeenCalledOnce());
  });

  it('shows the API error inline and keeps the modal open when create_project rejects', async () => {
    const onClose = vi.fn();
    mockInvoke.mockRejectedValue(new Error('No free project slot. Subscribe at postlane.dev/billing'));
    render(<AddWorkspaceModal onClose={onClose} onCreated={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'New Workspace' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('shows the no-free-slot message when create_project returns no_free_slot error', async () => {
    mockInvoke.mockRejectedValue(new Error('No free project slot. Subscribe at postlane.dev/billing'));
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'New Workspace' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    expect(await screen.findByText(/no free workspace slot/i)).toBeInTheDocument();
  });

  it('calls onClose and does not call create_project when Cancel is clicked', () => {
    const onClose = vi.fn();
    render(<AddWorkspaceModal onClose={onClose} onCreated={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledOnce();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('test_generic_api_error_shows_failed_to_create_workspace_message', async () => {
    mockInvoke.mockRejectedValue(new Error('Network timeout'));
    render(<AddWorkspaceModal onClose={vi.fn()} onCreated={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'My Workspace' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Failed to create workspace: Network timeout');
  });
});
