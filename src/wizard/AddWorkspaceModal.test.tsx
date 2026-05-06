// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import AddWorkspaceModal from './AddWorkspaceModal';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
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
    mockInvoke.mockRejectedValue(new Error('no_free_slot'));
    render(<AddWorkspaceModal onClose={onClose} onCreated={vi.fn()} />);
    fireEvent.change(screen.getByRole('textbox', { name: /workspace name/i }), { target: { value: 'New Workspace' } });
    fireEvent.click(screen.getByRole('button', { name: /create workspace/i }));
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('calls onClose and does not call create_project when Cancel is clicked', () => {
    const onClose = vi.fn();
    render(<AddWorkspaceModal onClose={onClose} onCreated={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledOnce();
    expect(mockInvoke).not.toHaveBeenCalled();
  });
});
