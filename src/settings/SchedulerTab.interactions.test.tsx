// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab from './SchedulerTab';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('./MastodonOAuthPanel', () => ({ default: () => <div data-testid="mastodon-panel" /> }));
vi.mock('./SubstackNotesPanel', () => ({ default: () => <div data-testid="substack-panel" /> }));
vi.mock('./WebhookPanel', () => ({ default: () => <div data-testid="webhook-panel" /> }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

function getZernioCard(): Element {
  const card = document.querySelector('[data-provider="zernio"]');
  expect(card).not.toBeNull();
  return card as Element;
}

function getButtonInCard(card: Element, label: string): Element {
  const btn = Array.from(card.querySelectorAll('button')).find((b) => b.textContent?.includes(label));
  expect(btn).toBeDefined();
  return btn as Element;
}

function scopedZernioCreds() {
  mockInvoke.mockImplementation(async (cmd: unknown, args: unknown) => {
    if (cmd === 'get_scheduler_credential') {
      if ((args as { provider: string }).provider === 'zernio') return '••••abcd';
      throw new Error('not found');
    }
    if (cmd === 'get_scheduler_usage') return null;
    return null;
  });
}

async function openRemoveDialogForZernio(): Promise<Element> {
  const card = getZernioCard();
  fireEvent.click(getButtonInCard(card, 'Remove'));
  const dialog = screen.getByRole('dialog');
  const textInput = dialog.querySelector('input[type="text"]');
  expect(textInput).not.toBeNull();
  fireEvent.change(textInput as Element, { target: { value: 'zernio' } });
  return screen.getByRole('button', { name: 'Remove' });
}

beforeEach(() => vi.clearAllMocks());

// ──────────────────────────────────────────────
// Change flow
// ──────────────────────────────────────────────

describe('SchedulerProviderCard — change flow', () => {
  beforeEach(() => scopedZernioCreds());

  it('test_shows_api_key_input_after_clicking_change', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(screen.getByRole('button', { name: 'Change' }));
    expect(screen.getByPlaceholderText('API key')).toBeInTheDocument();
  });
});

// ──────────────────────────────────────────────
// Test button flow
// ──────────────────────────────────────────────

type TestSchedulerImpl = (cmd: unknown, args: unknown) => Promise<unknown>;

function makeTestMock(testSchedulerImpl: TestSchedulerImpl) {
  mockInvoke.mockImplementation(async (cmd: unknown, args: unknown) => {
    if (cmd === 'get_scheduler_credential') {
      if ((args as { provider: string }).provider === 'zernio') return '••••abcd';
      throw new Error('not found');
    }
    if (cmd === 'get_scheduler_usage') return null;
    return testSchedulerImpl(cmd, args);
  });
}

describe('SchedulerProviderCard — test flow', () => {
  it('test_shows_success_checkmark_after_successful_test', async () => {
    makeTestMock(async (cmd) => (cmd === 'test_scheduler' ? null : null));
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Test'));
    await waitFor(() => expect(screen.getByText('✓')).toBeInTheDocument());
  });

  it('test_shows_error_text_after_failed_test', async () => {
    makeTestMock(async (cmd) => {
      if (cmd === 'test_scheduler') throw new Error('invalid key');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Test'));
    await waitFor(() => expect(screen.getByText('invalid key')).toBeInTheDocument());
  });

  it('test_shows_generic_error_text_when_error_is_not_an_Error_instance', async () => {
    makeTestMock(async (cmd) => {
      if (cmd === 'test_scheduler') return Promise.reject('string-error');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Test'));
    await waitFor(() => expect(screen.getByText('Test failed')).toBeInTheDocument());
  });

  it('test_test_button_is_disabled_while_testing', async () => {
    let resolveTest!: () => void;
    makeTestMock(async (cmd) => {
      if (cmd === 'test_scheduler') return new Promise<null>((res) => { resolveTest = () => res(null); });
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    const testButton = getButtonInCard(getZernioCard(), 'Test');
    fireEvent.click(testButton);
    await waitFor(() => expect(testButton).toBeDisabled());
    resolveTest();
    await waitFor(() => expect(testButton).not.toBeDisabled());
  });
});

// ──────────────────────────────────────────────
// Remove key dialog — open and close
// ──────────────────────────────────────────────

describe('RemoveKeyDialog — open and close', () => {
  beforeEach(() => scopedZernioCreds());

  it('test_remove_dialog_opens_on_remove_button_click', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(screen.getByText(/Remove zernio API key/i)).toBeInTheDocument();
  });

  it('test_remove_dialog_closes_on_cancel_click', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('test_remove_dialog_closes_on_background_click', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    const backdrop = document.querySelector('.modal-background');
    expect(backdrop).not.toBeNull();
    fireEvent.click(backdrop as Element);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('test_remove_dialog_closes_on_close_button_click', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('test_remove_dialog_closes_on_escape_key', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('test_remove_dialog_stays_open_on_non_escape_keydown', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    fireEvent.keyDown(document, { key: 'Enter' });
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });
});

// ──────────────────────────────────────────────
// Remove key dialog — input validation
// ──────────────────────────────────────────────

describe('RemoveKeyDialog — input validation', () => {
  beforeEach(() => scopedZernioCreds());

  it('test_remove_dialog_confirm_button_disabled_when_input_does_not_match', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    expect(screen.getByRole('button', { name: 'Remove' })).toBeDisabled();
  });

  it('test_remove_dialog_confirm_button_enabled_when_input_matches_provider', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    const dialog = screen.getByRole('dialog');
    const textInput = dialog.querySelector('input[type="text"]');
    expect(textInput).not.toBeNull();
    fireEvent.change(textInput as Element, { target: { value: 'zernio' } });
    expect(screen.getByRole('button', { name: 'Remove' })).not.toBeDisabled();
  });

  it('test_remove_dialog_shows_provider_name_in_warning_text', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    expect(screen.getByRole('dialog')).toHaveTextContent(/Any repos using zernio will stop working/i);
  });
});

// ──────────────────────────────────────────────
// Remove key dialog — IPC confirmation
// ──────────────────────────────────────────────

type DeleteImpl = (cmd: unknown) => Promise<unknown>;

function makeDeleteMock(deleteImpl: DeleteImpl) {
  mockInvoke.mockImplementation(async (cmd: unknown, args: unknown) => {
    if (cmd === 'get_scheduler_credential') {
      if ((args as { provider: string }).provider === 'zernio') return '••••abcd';
      throw new Error('not found');
    }
    if (cmd === 'get_scheduler_usage') return null;
    return deleteImpl(cmd);
  });
}

describe('RemoveKeyDialog — confirm removal IPC', () => {
  it('test_calls_delete_scheduler_credential_on_confirm', async () => {
    makeDeleteMock(async () => null);
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(await openRemoveDialogForZernio());
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('delete_scheduler_credential', { provider: 'zernio' });
    });
  });

  it('test_dialog_closes_and_preview_clears_after_successful_removal', async () => {
    makeDeleteMock(async () => null);
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(await openRemoveDialogForZernio());
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    await waitFor(() => expect(screen.queryByText('••••abcd')).not.toBeInTheDocument());
  });

  it('test_shows_error_state_when_delete_credential_fails', async () => {
    makeDeleteMock(async (cmd) => {
      if (cmd === 'delete_scheduler_credential') throw new Error('keychain locked');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(await openRemoveDialogForZernio());
    await waitFor(() => expect(screen.getByText('keychain locked')).toBeInTheDocument());
  });

  it('test_shows_generic_error_when_remove_throws_non_error', async () => {
    makeDeleteMock(async (cmd) => {
      if (cmd === 'delete_scheduler_credential') return Promise.reject('raw-string-error');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(await openRemoveDialogForZernio());
    await waitFor(() => expect(screen.getByText('Failed to remove credential')).toBeInTheDocument());
  });
});

// ──────────────────────────────────────────────
// aria-hidden accessibility
// ──────────────────────────────────────────────

describe('SchedulerTab — accessibility', () => {
  beforeEach(() => scopedZernioCreds());

  it('test_main_content_has_aria_hidden_when_remove_dialog_is_open', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    expect(document.querySelector('[aria-hidden]')).toBeNull();
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    expect(document.querySelector('[aria-hidden="true"]')).not.toBeNull();
  });

  it('test_main_content_aria_hidden_removed_after_dialog_closes', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    fireEvent.click(getButtonInCard(getZernioCard(), 'Remove'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(document.querySelector('[aria-hidden="true"]')).toBeNull());
  });
});
