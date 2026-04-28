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
    render(
      <SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />,
    );
    expect(screen.getByText(/set up posting for my-blog/i)).toBeInTheDocument();
  });

  it('lists all seven scheduler providers', () => {
    render(
      <SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />,
    );
    expect(screen.getByText(/zernio/i)).toBeInTheDocument();
    expect(screen.getByText(/buffer/i)).toBeInTheDocument();
    expect(screen.getByText(/ayrshare/i)).toBeInTheDocument();
    expect(screen.getByText(/publer/i)).toBeInTheDocument();
    expect(screen.getByText(/outstand/i)).toBeInTheDocument();
    expect(screen.getByText(/substack notes/i)).toBeInTheDocument();
    expect(screen.getByText(/webhook/i)).toBeInTheDocument();
  });

  it('shows free tier notes for counted providers', () => {
    render(
      <SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />,
    );
    expect(screen.getByText(/10 posts\/month/i)).toBeInTheDocument();
    expect(screen.getByText(/1,000/i)).toBeInTheDocument();
  });

  it('shows a "Set up later" button', () => {
    render(
      <SchedulerSetupModal repoName="my-blog" onSetupLater={vi.fn()} />,
    );
    expect(screen.getByRole('button', { name: /set up later/i })).toBeInTheDocument();
  });
});

// §13.4.6 — "Set up later" dismisses and hands control back to the parent
describe('SchedulerSetupModal — Set up later', () => {
  it('calls onSetupLater when the button is clicked', () => {
    const onSetupLater = vi.fn();
    render(
      <SchedulerSetupModal repoName="my-blog" onSetupLater={onSetupLater} />,
    );
    fireEvent.click(screen.getByRole('button', { name: /set up later/i }));
    expect(onSetupLater).toHaveBeenCalledOnce();
  });
});

// §13.3.2 — provider selection and confirmation flow
describe('SchedulerSetupModal — provider selection (§13.3.2)', () => {
  it('clicking "Set up" for a provider calls onOpenSchedulerSettings', () => {
    const onOpenSchedulerSettings = vi.fn();
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={onOpenSchedulerSettings} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    expect(onOpenSchedulerSettings).toHaveBeenCalledWith('zernio');
  });

  it('shows Check button after clicking a provider', () => {
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    expect(screen.getByRole('button', { name: /check/i })).toBeInTheDocument();
  });

  it('shows Done button when has_scheduler_configured returns true', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_scheduler_configured') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    await waitFor(() => screen.getByRole('button', { name: /check/i }));
    fireEvent.click(screen.getByRole('button', { name: /check/i }));
    await waitFor(() => expect(screen.getByRole('button', { name: /done/i })).toBeInTheDocument());
  });

  it('calls onDone after Done is clicked', async () => {
    const onDone = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'has_scheduler_configured') return true;
      return null;
    });
    render(<SchedulerSetupModal repoName="test" repoId="r1" onSetupLater={vi.fn()} onOpenSchedulerSettings={vi.fn()} onDone={onDone} />);
    fireEvent.click(screen.getByRole('button', { name: /set up zernio/i }));
    await waitFor(() => screen.getByRole('button', { name: /check/i }));
    fireEvent.click(screen.getByRole('button', { name: /check/i }));
    await waitFor(() => screen.getByRole('button', { name: /done/i }));
    fireEvent.click(screen.getByRole('button', { name: /done/i }));
    await waitFor(() => expect(onDone).toHaveBeenCalledOnce());
  });
});
