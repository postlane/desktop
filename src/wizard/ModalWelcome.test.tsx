// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn().mockResolvedValue(undefined) }));
import { openUrl } from '@tauri-apps/plugin-opener';
const mockOpenUrl = vi.mocked(openUrl);

import ModalWelcome from './ModalWelcome';

beforeEach(() => { vi.clearAllMocks(); });

describe('ModalWelcome', () => {
  it('test_get_started_calls_onNext', async () => {
    const onNext = vi.fn();
    render(<ModalWelcome onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /get started/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_pricing_link_calls_openUrl', async () => {
    render(<ModalWelcome onNext={vi.fn()} />);
    await userEvent.click(screen.getByRole('link', { name: /see pricing/i }));
    expect(mockOpenUrl).toHaveBeenCalledWith('https://postlane.dev/pricing');
  });

  it('test_no_back_button', () => {
    render(<ModalWelcome onNext={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /back/i })).toBeNull();
  });
});
