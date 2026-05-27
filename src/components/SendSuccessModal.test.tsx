// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import { SendSuccessModal } from './SendSuccessModal';

describe('SendSuccessModal', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('renders a dialog role', () => {
    render(<SendSuccessModal platforms={['x', 'bluesky']} onClose={vi.fn()} />);
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('renders a status region inside the dialog', () => {
    render(<SendSuccessModal platforms={['x', 'bluesky']} onClose={vi.fn()} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('lists the platform names in uppercase inside the status region', () => {
    render(<SendSuccessModal platforms={['x', 'bluesky']} onClose={vi.fn()} />);
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent(/X/);
    expect(status).toHaveTextContent(/BLUESKY/);
  });

  it('shows "Sent" when no platforms are provided', () => {
    render(<SendSuccessModal platforms={[]} onClose={vi.fn()} />);
    expect(screen.getByRole('status')).toHaveTextContent('Sent');
  });

  it('calls onClose when the modal background is clicked', () => {
    const onClose = vi.fn();
    render(<SendSuccessModal platforms={['x']} onClose={onClose} />);
    fireEvent.click(screen.getByTestId('modal-background'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('does not call onClose before autoDismissMs elapses', () => {
    const onClose = vi.fn();
    render(<SendSuccessModal platforms={['x']} onClose={onClose} autoDismissMs={2500} />);
    vi.advanceTimersByTime(2499);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('auto-dismisses by calling onClose after autoDismissMs', () => {
    const onClose = vi.fn();
    render(<SendSuccessModal platforms={['x']} onClose={onClose} autoDismissMs={2500} />);
    vi.advanceTimersByTime(2500);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
