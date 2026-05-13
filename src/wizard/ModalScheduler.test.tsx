// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

import ModalScheduler from './ModalScheduler';

const defaultProps = {
  workspaceId: 'ws-1',
  workspaceName: 'Test Workspace',
  onNext: vi.fn(),
  onBack: vi.fn(),
  setSchedulerLinked: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_connected_providers') return [];
    return undefined;
  });
});

describe('ModalScheduler — picker', () => {
  it('test_renders_provider_options_and_skip', () => {
    render(<ModalScheduler {...defaultProps} />);
    expect(screen.getByRole('button', { name: /zernio/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /upload post/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /skip/i })).toBeDefined();
  });

  it('test_selecting_zernio_opens_key_entry', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('test_selecting_upload_post_opens_key_entry', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /upload post/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('test_skip_calls_onNext_with_scheduler_not_linked', async () => {
    const setSchedulerLinked = vi.fn();
    const onNext = vi.fn();
    render(<ModalScheduler {...defaultProps} setSchedulerLinked={setSchedulerLinked} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /skip/i }));
    expect(setSchedulerLinked).toHaveBeenCalledWith(false);
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_cancel_in_key_entry_returns_to_picker', async () => {
    render(<ModalScheduler {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.getByRole('button', { name: /zernio/i })).toBeDefined();
  });
});

async function connectZernio(overrides: { onNext?: () => void; setSchedulerLinked?: (b: boolean) => void } = {}) {
  render(<ModalScheduler {...defaultProps} {...overrides} />);
  await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
  await userEvent.type(screen.getByRole('textbox'), 'my-api-key');
  await userEvent.click(screen.getByRole('button', { name: /connect/i }));
  await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
}

describe('ModalScheduler — after connecting first provider', () => {
  it('test_stays_on_picker_without_advancing', async () => {
    const onNext = vi.fn();
    await connectZernio({ onNext });
    expect(onNext).not.toHaveBeenCalled();
  });

  it('test_connected_provider_button_shows_connected_badge', async () => {
    await connectZernio();
    expect(screen.getByRole('button', { name: /zernio/i }).textContent).toContain('Connected');
  });

  it('test_connected_provider_button_remains_clickable', async () => {
    await connectZernio();
    expect((screen.getByRole('button', { name: /zernio/i }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('test_skip_is_hidden', async () => {
    await connectZernio();
    expect(screen.queryByRole('button', { name: /skip/i })).toBeNull();
  });

  it('test_next_button_is_visible', async () => {
    await connectZernio();
    expect(screen.getByRole('button', { name: /next/i })).toBeDefined();
  });

  it('test_next_button_calls_onNext', async () => {
    const onNext = vi.fn();
    await connectZernio({ onNext });
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_sets_scheduler_linked_true', async () => {
    const setSchedulerLinked = vi.fn();
    await connectZernio({ setSchedulerLinked });
    expect(setSchedulerLinked).toHaveBeenCalledWith(true);
  });

  it('test_second_provider_button_remains_enabled', async () => {
    await connectZernio();
    expect((screen.getByRole('button', { name: /upload post/i }) as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('ModalScheduler — pre-connected providers', () => {
  function setupPreConnected(providers: string[]) {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_connected_providers') return providers;
      return undefined;
    });
  }

  it('shows Connected badge on button for pre-connected provider', async () => {
    setupPreConnected(['zernio']);
    render(<ModalScheduler {...defaultProps} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /zernio/i }).textContent).toContain('Connected'),
    );
  });

  it('pre-connected provider button is not disabled', async () => {
    setupPreConnected(['zernio']);
    render(<ModalScheduler {...defaultProps} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /zernio/i }).textContent).toContain('Connected'),
    );
    expect((screen.getByRole('button', { name: /zernio/i }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('clicking a pre-connected button still opens key entry', async () => {
    setupPreConnected(['zernio']);
    render(<ModalScheduler {...defaultProps} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /zernio/i }).textContent).toContain('Connected'),
    );
    await userEvent.click(screen.getByRole('button', { name: /zernio/i }));
    expect(screen.getByRole('textbox')).toBeDefined();
  });

  it('Next is visible immediately when a provider is pre-connected', async () => {
    setupPreConnected(['zernio']);
    render(<ModalScheduler {...defaultProps} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /next/i })).toBeDefined());
    expect(screen.queryByRole('button', { name: /skip/i })).toBeNull();
  });

  it('unconnected provider button shows no Connected badge', async () => {
    setupPreConnected(['zernio']);
    render(<ModalScheduler {...defaultProps} />);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /zernio/i }).textContent).toContain('Connected'),
    );
    expect(screen.getByRole('button', { name: /upload post/i }).textContent).not.toContain('Connected');
  });
});
