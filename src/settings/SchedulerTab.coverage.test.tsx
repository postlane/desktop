// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerTab, { UsageBadge } from './SchedulerTab';
import type { UsageResponse } from './SchedulerTab';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('./MastodonOAuthPanel', () => ({ default: () => <div data-testid="mastodon-panel" /> }));
vi.mock('./SubstackNotesPanel', () => ({ default: () => <div data-testid="substack-panel" /> }));
vi.mock('./WebhookPanel', () => ({ default: () => <div data-testid="webhook-panel" /> }));

import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

function makeDefaultMocks(credOverrides: Record<string, string> = {}) {
  mockInvoke.mockImplementation(async (cmd: unknown, args: unknown) => {
    if (cmd === 'get_scheduler_credential') {
      const { provider } = args as { provider: string };
      if (provider in credOverrides) return credOverrides[provider];
      throw new Error('not found');
    }
    if (cmd === 'get_scheduler_usage') return null;
    if (cmd === 'get_mastodon_connected_instance') return null;
    return null;
  });
}

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

beforeEach(() => vi.clearAllMocks());

// ──────────────────────────────────────────────
// UsageBadge — isolated unit tests
// ──────────────────────────────────────────────

describe('UsageBadge — rendering logic', () => {
  it('test_returns_null_when_usage_undefined', () => {
    const { container } = render(<UsageBadge usage={undefined} />);
    expect(container.firstChild).toBeNull();
  });

  it('test_returns_null_when_limit_is_null', () => {
    const usage: UsageResponse = { provider: 'publer', count: 5, limit: null, month: 4, year: 2026 };
    const { container } = render(<UsageBadge usage={usage} />);
    expect(container.firstChild).toBeNull();
  });

  it('test_shows_normal_usage_text_below_80_percent', () => {
    const usage: UsageResponse = { provider: 'publer', count: 3, limit: 10, month: 4, year: 2026 };
    render(<UsageBadge usage={usage} />);
    expect(screen.getByText(/3\/10 posts used this month/i)).toBeInTheDocument();
  });

  it('test_shows_near_limit_warning_at_exactly_80_percent', () => {
    const usage: UsageResponse = { provider: 'publer', count: 8, limit: 10, month: 4, year: 2026 };
    render(<UsageBadge usage={usage} />);
    expect(screen.getByText(/approaching limit/i)).toBeInTheDocument();
  });

  it('test_shows_at_limit_danger_text_when_count_equals_limit', () => {
    const usage: UsageResponse = { provider: 'publer', count: 10, limit: 10, month: 4, year: 2026 };
    render(<UsageBadge usage={usage} />);
    expect(screen.getByText(/limit reached/i)).toBeInTheDocument();
  });

  it('test_shows_at_limit_danger_text_when_count_exceeds_limit', () => {
    const usage: UsageResponse = { provider: 'publer', count: 12, limit: 10, month: 4, year: 2026 };
    render(<UsageBadge usage={usage} />);
    expect(screen.getByText(/limit reached/i)).toBeInTheDocument();
  });
});

// ──────────────────────────────────────────────
// SchedulerProviderCard — configured credential
// ──────────────────────────────────────────────

describe('SchedulerProviderCard — when credential is configured', () => {
  beforeEach(() => makeDefaultMocks({ zernio: '••••abcd' }));

  it('test_shows_credential_preview_text_when_configured', async () => {
    render(<SchedulerTab />);
    expect(await screen.findByText('••••abcd')).toBeInTheDocument();
  });

  it('test_shows_test_button_when_configured', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    expect(getZernioCard()).toHaveTextContent('Test');
  });

  it('test_shows_change_button_when_configured', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    expect(getZernioCard()).toHaveTextContent('Change');
  });

  it('test_shows_remove_button_when_configured', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    expect(getZernioCard()).toHaveTextContent('Remove');
  });

  it('test_does_not_show_add_button_when_configured', async () => {
    render(<SchedulerTab />);
    await screen.findByText('••••abcd');
    expect(getZernioCard()).not.toHaveTextContent('+ Add');
  });
});

describe('SchedulerProviderCard — when credential is not configured', () => {
  beforeEach(() => makeDefaultMocks({}));

  it('test_shows_not_configured_text_when_no_credential', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(getZernioCard()).toHaveTextContent('not configured');
  });

  it('test_shows_add_button_when_not_configured', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(getZernioCard()).toHaveTextContent('+ Add');
  });
});

// ──────────────────────────────────────────────
// Adding credential flow
// ──────────────────────────────────────────────

describe('SchedulerProviderCard — adding credential flow', () => {
  beforeEach(() => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return null;
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
  });

  it('test_shows_api_key_input_after_clicking_add', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    fireEvent.click(getButtonInCard(getZernioCard(), '+ Add'));
    expect(screen.getByPlaceholderText('API key')).toBeInTheDocument();
  });

  it('test_shows_save_and_cancel_buttons_when_adding', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    fireEvent.click(getButtonInCard(getZernioCard(), '+ Add'));
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
  });

  it('test_hides_api_key_input_after_clicking_cancel', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    fireEvent.click(getButtonInCard(getZernioCard(), '+ Add'));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByPlaceholderText('API key')).not.toBeInTheDocument();
  });

  it('test_saves_credential_on_save_click', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    fireEvent.click(getButtonInCard(getZernioCard(), '+ Add'));
    fireEvent.change(screen.getByPlaceholderText('API key'), { target: { value: 'my-secret-key' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', {
        provider: 'zernio',
        apiKey: 'my-secret-key',
      });
    });
  });

  it('test_does_not_call_save_when_key_input_is_empty', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    fireEvent.click(getButtonInCard(getZernioCard(), '+ Add'));
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(mockInvoke).not.toHaveBeenCalledWith('save_scheduler_credential', expect.anything());
    });
  });
});

// ──────────────────────────────────────────────
// handleSave — masked preview and error handling
// ──────────────────────────────────────────────

describe('handleSave — credential save result', () => {
  it('test_shows_masked_preview_after_saving_key', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return null;
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    const card = getZernioCard();
    fireEvent.click(getButtonInCard(card, '+ Add'));
    fireEvent.change(screen.getByPlaceholderText('API key'), { target: { value: 'supersecretkey' } });
    fireEvent.click(getButtonInCard(card, 'Save'));
    await waitFor(() => expect(card).toHaveTextContent('••••tkey'));
  });

  it('test_hides_add_form_after_successful_save', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return null;
      if (cmd === 'save_scheduler_credential') return null;
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    const card = getZernioCard();
    fireEvent.click(getButtonInCard(card, '+ Add'));
    fireEvent.change(screen.getByPlaceholderText('API key'), { target: { value: 'supersecretkey' } });
    fireEvent.click(getButtonInCard(card, 'Save'));
    await waitFor(() => expect(screen.queryByPlaceholderText('API key')).not.toBeInTheDocument());
  });

  it('test_does_not_crash_when_save_credential_throws', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return null;
      if (cmd === 'save_scheduler_credential') throw new Error('keychain error');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    const card = getZernioCard();
    fireEvent.click(getButtonInCard(card, '+ Add'));
    fireEvent.change(screen.getByPlaceholderText('API key'), { target: { value: 'myapikey1234' } });
    fireEvent.click(getButtonInCard(card, 'Save'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_scheduler_credential', expect.anything());
    });
    expect(screen.getByRole('heading', { name: /^Scheduler$/i })).toBeInTheDocument();
  });
});

// ──────────────────────────────────────────────
// Panel sub-components and static content
// ──────────────────────────────────────────────

describe('SchedulerTab — sub-panels and static content', () => {
  beforeEach(() => makeDefaultMocks({}));

  it('test_mastodon_panel_is_rendered', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.getByTestId('mastodon-panel')).toBeInTheDocument();
  });

  it('test_substack_panel_is_rendered', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.getByTestId('substack-panel')).toBeInTheDocument();
  });

  it('test_webhook_panel_is_rendered', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.getByTestId('webhook-panel')).toBeInTheDocument();
  });

  it('test_keychain_note_is_visible', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.getByText(/macOS Keychain/i)).toBeInTheDocument();
  });
});

// ──────────────────────────────────────────────
// Provider notes
// ──────────────────────────────────────────────

describe('SchedulerTab — provider notes', () => {
  beforeEach(() => makeDefaultMocks({}));

  it('test_upload_post_note_is_shown', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/upload.post/i);
    expect(screen.getByText(/10 uploads\/month free/i)).toBeInTheDocument();
  });

  it('test_publer_note_is_shown', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/publer/i);
    expect(screen.getByText(/API access requires a paid plan/i)).toBeInTheDocument();
  });

  it('test_outstand_note_is_shown', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/outstand/i);
    expect(screen.getByText(/\$5\/month for 1,000 posts/i)).toBeInTheDocument();
  });

  it('test_zernio_has_no_note', async () => {
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(getZernioCard().querySelector('.is-size-7.has-text-grey.mt-1')).toBeNull();
  });
});

// ──────────────────────────────────────────────
// loadSchedulerCreds — exported function
// ──────────────────────────────────────────────

describe('loadSchedulerCreds — exported function', () => {
  it('test_skips_provider_when_invoke_throws', async () => {
    mockInvoke.mockRejectedValue(new Error('not found'));
    const { loadSchedulerCreds } = await import('./SchedulerTab');
    const onCred = vi.fn();
    await loadSchedulerCreds(() => false, onCred);
    expect(onCred).not.toHaveBeenCalled();
  });

  it('test_calls_on_cred_for_each_configured_provider', async () => {
    mockInvoke.mockResolvedValue('preview-value');
    const { loadSchedulerCreds } = await import('./SchedulerTab');
    const onCred = vi.fn();
    await loadSchedulerCreds(() => false, onCred);
    expect(onCred).toHaveBeenCalledTimes(4);
  });
});

// ──────────────────────────────────────────────
// Usage fetch — error handling and cancellation
// ──────────────────────────────────────────────

describe('SchedulerTab — usage fetch behaviour', () => {
  it('test_renders_without_crashing_when_usage_fetch_fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') throw new Error('network error');
      return null;
    });
    render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    expect(screen.queryByText(/posts used this month/i)).not.toBeInTheDocument();
  });

  it('test_does_not_set_usage_state_when_component_unmounts_before_fetch_resolves', async () => {
    let resolveUsage!: (v: unknown) => void;
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_scheduler_credential') throw new Error('not found');
      if (cmd === 'get_scheduler_usage') return new Promise((res) => { resolveUsage = res; });
      return null;
    });
    const { unmount } = render(<SchedulerTab />);
    await screen.findByText(/zernio/i);
    unmount();
    resolveUsage({ provider: 'publer', count: 5, limit: 10, month: 4, year: 2026 });
  });
});
