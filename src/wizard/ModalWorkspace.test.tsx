// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

import ModalWorkspace from './ModalWorkspace';

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalWorkspace', () => {
  it('test_next_disabled_when_name_empty', () => {
    render(<ModalWorkspace onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('test_stores_workspace_id_on_success', async () => {
    const onNext = vi.fn();
    mockInvoke.mockResolvedValue({ project_id: 'proj-abc', name: 'Acme', workspace_type: 'personal' });
    render(<ModalWorkspace onNext={onNext} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await userEvent.type(screen.getByRole('textbox'), 'Acme');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'Acme', workspaceType: 'personal' });
      expect(onNext).toHaveBeenCalledWith('proj-abc');
    });
  });

  it('test_shows_error_on_network_failure', async () => {
    mockInvoke.mockRejectedValue(new Error('network timeout'));
    render(<ModalWorkspace onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await userEvent.type(screen.getByRole('textbox'), 'Acme');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeDefined();
    });
  });

  it('test_calls_onPricingGate_on_402', async () => {
    const onPricingGate = vi.fn();
    mockInvoke.mockRejectedValue(new Error('no_free_slot'));
    render(<ModalWorkspace onNext={vi.fn()} onBack={vi.fn()} onPricingGate={onPricingGate} />);
    await userEvent.type(screen.getByRole('textbox'), 'Acme');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onPricingGate).toHaveBeenCalledOnce());
  });

  it('test_workspace_type_defaults_to_personal', () => {
    render(<ModalWorkspace onNext={vi.fn()} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('personal');
  });

  it('test_passes_workspace_type_to_invoke', async () => {
    const onNext = vi.fn();
    mockInvoke.mockResolvedValue({ project_id: 'proj-xyz', name: 'Acme', workspace_type: 'organization' });
    render(<ModalWorkspace onNext={onNext} onBack={vi.fn()} onPricingGate={vi.fn()} />);
    await userEvent.type(screen.getByRole('textbox'), 'Acme');
    await userEvent.selectOptions(screen.getByRole('combobox'), 'organization');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_project', { name: 'Acme', workspaceType: 'organization' });
    });
  });
});
