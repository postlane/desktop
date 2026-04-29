// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import SchedulerSetupModal from './SchedulerSetupModal';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(null);
});

describe('SchedulerSetupModal — rendering', () => {
  it('shows the repo name in the title', () => {
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />);
    expect(screen.getByText(/set up posting for my-blog/i)).toBeInTheDocument();
  });

  it('lists all seven scheduler providers', () => {
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />);
    expect(screen.getByText(/zernio/i)).toBeInTheDocument();
    expect(screen.getByText(/buffer/i)).toBeInTheDocument();
    expect(screen.getByText(/ayrshare/i)).toBeInTheDocument();
    expect(screen.getByText(/publer/i)).toBeInTheDocument();
    expect(screen.getByText(/outstand/i)).toBeInTheDocument();
    expect(screen.getByText(/substack notes/i)).toBeInTheDocument();
    expect(screen.getByText(/webhook/i)).toBeInTheDocument();
  });

  it('shows free tier notes', () => {
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />);
    expect(screen.getByText(/10 posts\/month/i)).toBeInTheDocument();
    expect(screen.getByText(/1,000/i)).toBeInTheDocument();
  });

  it('shows a "Set up later" button', () => {
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />);
    expect(screen.getByRole('button', { name: /set up later/i })).toBeInTheDocument();
  });

  it('does not show Done button before any provider is configured', () => {
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /^done$/i })).not.toBeInTheDocument();
  });
});

describe('SchedulerSetupModal — Set up later', () => {
  it('calls onSetupLater when the button is clicked', () => {
    const onSetupLater = vi.fn();
    render(<SchedulerSetupModal repoName="my-blog" onSetupLater={onSetupLater} />);
    fireEvent.click(screen.getByRole('button', { name: /set up later/i }));
    expect(onSetupLater).toHaveBeenCalledOnce();
  });
});

describe('SchedulerSetupModal — single provider (§13.3.2)', () => {
  it('clicking "Set up" calls onOpenSchedulerSettings with the provider key', () => {
    const onOpenSchedulerSettings = vi.fn();
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={onOpenSchedulerSettings} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    expect(onOpenSchedulerSettings).toHaveBeenCalledWith('zernio');
  });

  it('shows a Check button for the provider after clicking Set up', () => {
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    expect(screen.getByRole('button', { name: /check zernio/i })).toBeInTheDocument();
  });

  it('shows Done button after has_provider_credential returns true', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /^done$/i })).toBeInTheDocument());
  });

  it('calls update_scheduler_config with fallbackOrder and onDone', async () => {
    const onDone = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={onDone} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /^done$/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('update_scheduler_config', expect.objectContaining({ fallbackOrder: ['zernio'] })));
    await waitFor(() => expect(onDone).toHaveBeenCalledOnce());
  });
});

describe('SchedulerSetupModal — multiple providers (§13.3.2 optional)', () => {
  it('shows a priority badge when a provider is configured', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => expect(screen.getByText(/#1/)).toBeInTheDocument());
  });

  it('allows a second provider to be added in priority order', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => screen.getByText(/#1/));
    fireEvent.click(screen.getByRole('button', { name: /set up publer/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check publer/i }));
    await waitFor(() => expect(screen.getByText(/#2/)).toBeInTheDocument());
  });

  it('calls update_scheduler_config with full fallback_order when two providers configured', async () => {
    const onDone = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={onDone} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => screen.getByText(/#1/));
    fireEvent.click(screen.getByRole('button', { name: /set up publer/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check publer/i }));
    await waitFor(() => screen.getByText(/#2/));
    fireEvent.click(screen.getByRole('button', { name: /^done$/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('update_scheduler_config',
      expect.objectContaining({ fallbackOrder: ['zernio', 'publer'] })));
  });

  it('removes a provider from the list when Remove is clicked', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => screen.getByText(/#1/));
    fireEvent.click(screen.getByRole('button', { name: /remove zernio/i }));
    await waitFor(() => expect(screen.queryByText(/#1/)).not.toBeInTheDocument());
  });
});

describe('SchedulerSetupModal — Remove deletes from keyring (§fix)', () => {
  it('calls delete_scheduler_credential when Remove is clicked on a configured provider', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_provider_credential') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    fireEvent.click(await screen.findByRole('button', { name: /check zernio/i }));
    await waitFor(() => screen.getByText(/#1/));
    fireEvent.click(screen.getByRole('button', { name: /remove zernio/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'delete_scheduler_credential',
        expect.objectContaining({ provider: 'zernio' }),
      ),
    );
  });
});
