// SPDX-License-Identifier: BUSL-1.1
// 9.4 — Settings → Mastodon OAuth panel

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import MastodonOAuthPanel from './MastodonOAuthPanel';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(openUrl);

beforeEach(() => vi.clearAllMocks());

describe('MastodonOAuthPanel — instance field validation', () => {
  it('renders the instance domain input', () => {
    render(<MastodonOAuthPanel />);
    expect(screen.getByPlaceholderText(/mastodon\.social/i)).toBeInTheDocument();
  });

  it('shows an error when input contains "://" on Test instance', () => {
    render(<MastodonOAuthPanel />);
    const input = screen.getByPlaceholderText(/mastodon\.social/i);
    fireEvent.change(input, { target: { value: 'https://mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    expect(screen.getByText(/hostname only/i)).toBeInTheDocument();
  });

  it('does not show an error for a bare hostname', async () => {
    mockInvoke.mockResolvedValueOnce(500);
    render(<MastodonOAuthPanel />);
    const input = screen.getByPlaceholderText(/mastodon\.social/i);
    fireEvent.change(input, { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => expect(screen.queryByText(/hostname only/i)).not.toBeInTheDocument());
  });
});

async function validateAndReadyConnect(instance = 'mastodon.social') {
  fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: instance } });
  fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
  await waitFor(() => screen.getByRole('button', { name: /connect/i, hidden: false }));
}

describe('MastodonOAuthPanel — Connect flow', () => {
  it('calls register_mastodon_app with the instance on Connect', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc');
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('register_mastodon_app', { instance: 'mastodon.social' }));
  });

  it('opens the auth URL in the browser after Connect succeeds', async () => {
    const authUrl = 'https://mastodon.social/oauth/authorize?client_id=abc';
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockResolvedValueOnce(authUrl);
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(mockOpen).toHaveBeenCalledWith(authUrl));
  });

  it('shows the auth code input field after Connect succeeds', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc');
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.getByPlaceholderText(/paste the code/i)).toBeInTheDocument());
  });

  it('shows inline error when Connect fails', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockRejectedValueOnce(new Error('Network error'));
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.getByText(/network error/i)).toBeInTheDocument());
  });
});

describe('MastodonOAuthPanel — Save (code exchange) flow', () => {
  it('calls exchange_mastodon_code with instance and code on Save', async () => {
    mockInvoke
      .mockResolvedValueOnce(500) // test-instance
      .mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc')
      .mockResolvedValueOnce('alice');
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByPlaceholderText(/paste the code/i));
    fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('exchange_mastodon_code', { instance: 'mastodon.social', code: 'abc123' })
    );
  });

  it('transitions to connected state showing @acct after Save', async () => {
    mockInvoke
      .mockResolvedValueOnce(500) // test-instance
      .mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc')
      .mockResolvedValueOnce('alice');
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByPlaceholderText(/paste the code/i));
    fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => expect(screen.getByText(/@alice/)).toBeInTheDocument());
  });
});

describe('MastodonOAuthPanel — Disconnect', () => {
  async function reachConnectedState() {
    mockInvoke
      .mockResolvedValueOnce(500) // test-instance
      .mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc')
      .mockResolvedValueOnce('alice');
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByPlaceholderText(/paste the code/i));
    fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => screen.getByRole('button', { name: /disconnect/i }));
  }

  it('calls disconnect_mastodon and resets to idle state', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true);
    mockInvoke.mockResolvedValueOnce(null);
    await reachConnectedState();
    fireEvent.click(screen.getByRole('button', { name: /disconnect/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('disconnect_mastodon', { instance: 'mastodon.social' })
    );
    await waitFor(() => expect(screen.getByPlaceholderText(/mastodon\.social/i)).toBeInTheDocument());
  });

  // Issue 9 — confirm dialog must appear before disconnect to prevent accidental credential removal
  it('shows a confirmation dialog before disconnecting', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValueOnce(false);
    await reachConnectedState();
    fireEvent.click(screen.getByRole('button', { name: /disconnect/i }));
    expect(confirmSpy).toHaveBeenCalled();
  });

  it('does not call disconnect_mastodon when confirmation is cancelled', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(false);
    await reachConnectedState();
    fireEvent.click(screen.getByRole('button', { name: /disconnect/i }));
    expect(mockInvoke).not.toHaveBeenCalledWith('disconnect_mastodon', expect.anything());
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument();
  });
});

// Issue 10 — openUrl errors must be surfaced to the user, not silently swallowed
describe('MastodonOAuthPanel — openUrl error handling', () => {
  it('shows an inline error when openUrl throws after Connect', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc');
    mockOpen.mockRejectedValueOnce(new Error('Could not open browser'));
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.getByText(/could not open browser/i)).toBeInTheDocument());
  });

  it('stays on idle step when openUrl throws', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc');
    mockOpen.mockRejectedValueOnce(new Error('Could not open browser'));
    render(<MastodonOAuthPanel />);
    await validateAndReadyConnect();
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.queryByPlaceholderText(/paste the code/i)).not.toBeInTheDocument());
  });
});

// §review-product-medium — instance real-time validation before Connect
describe('MastodonOAuthPanel — instance validation (§review-product-medium)', () => {
  it('renders a "Test instance" button in the idle form', () => {
    render(<MastodonOAuthPanel />);
    expect(screen.getByRole('button', { name: /test instance/i })).toBeInTheDocument();
  });

  it('Connect button is disabled until instance is validated', () => {
    render(<MastodonOAuthPanel />);
    expect(screen.getByRole('button', { name: /connect/i })).toBeDisabled();
  });

  it('calls get_mastodon_char_limit when Test instance is clicked', async () => {
    mockInvoke.mockResolvedValueOnce(500);
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('get_mastodon_char_limit', { instance: 'mastodon.social' })
    );
  });

  it('shows "Valid" indicator after successful test', async () => {
    mockInvoke.mockResolvedValueOnce(500);
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => expect(screen.getByText(/valid/i)).toBeInTheDocument());
  });

  it('shows "Instance not found" when test fails', async () => {
    mockInvoke.mockRejectedValueOnce(new Error('could not reach host'));
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'bad.instance' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => expect(screen.getByText(/instance not found/i)).toBeInTheDocument());
  });

  it('enables Connect after a successful test', async () => {
    mockInvoke.mockResolvedValueOnce(500);
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /connect/i })).toBeEnabled());
  });

  it('resets validation state when instance input changes', async () => {
    mockInvoke.mockResolvedValueOnce(500);
    render(<MastodonOAuthPanel />);
    const input = screen.getByPlaceholderText(/mastodon\.social/i);
    fireEvent.change(input, { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => screen.getByText(/valid/i));
    fireEvent.change(input, { target: { value: 'other.social' } });
    expect(screen.queryByText(/valid/i)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /connect/i })).toBeDisabled();
  });
});

describe('MastodonOAuthPanel — Connect error non-Error branch', () => {
  it('shows a string error when register_mastodon_app rejects with a non-Error', async () => {
    mockInvoke.mockResolvedValueOnce(500); // test-instance
    mockInvoke.mockRejectedValueOnce('raw connect error');
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => screen.getByRole('button', { name: /connect/i, hidden: false }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => expect(screen.getByText('raw connect error')).toBeInTheDocument());
  });
});

describe('MastodonOAuthPanel — Save error non-Error branch', () => {
  async function reachCodeEntry() {
    mockInvoke.mockResolvedValueOnce(500);
    mockInvoke.mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc');
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => screen.getByRole('button', { name: /connect/i, hidden: false }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByPlaceholderText(/paste the code/i));
    fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'token123' } });
  }

  it('shows a string error when exchange_mastodon_code rejects with a non-Error', async () => {
    await reachCodeEntry();
    mockInvoke.mockRejectedValueOnce('raw string error');
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => expect(screen.getByText('raw string error')).toBeInTheDocument());
  });
});

describe('MastodonOAuthPanel — Disconnect error non-Error branch', () => {
  async function reachConnectedState() {
    mockInvoke
      .mockResolvedValueOnce(500)
      .mockResolvedValueOnce('https://mastodon.social/oauth/authorize?client_id=abc')
      .mockResolvedValueOnce('bob');
    render(<MastodonOAuthPanel />);
    fireEvent.change(screen.getByPlaceholderText(/mastodon\.social/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /test instance/i }));
    await waitFor(() => screen.getByRole('button', { name: /connect/i, hidden: false }));
    fireEvent.click(screen.getByRole('button', { name: /connect/i }));
    await waitFor(() => screen.getByPlaceholderText(/paste the code/i));
    fireEvent.change(screen.getByPlaceholderText(/paste the code/i), { target: { value: 'code99' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    await waitFor(() => screen.getByRole('button', { name: /disconnect/i }));
  }

  it('stays connected when disconnect_mastodon rejects with a non-Error string', async () => {
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true);
    await reachConnectedState();
    mockInvoke.mockRejectedValueOnce('disconnect failed string');
    fireEvent.click(screen.getByRole('button', { name: /disconnect/i }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /disconnect/i })).not.toBeDisabled(),
    );
    expect(screen.getByRole('button', { name: /disconnect/i })).toBeInTheDocument();
  });
});
