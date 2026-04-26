// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import TelemetryConsentModal from './TelemetryConsentModal';

describe('TelemetryConsentModal — keyboard and backdrop dismiss', () => {
  it('calls onDecline when Escape key is pressed', () => {
    const onDecline = vi.fn();
    render(<TelemetryConsentModal onAccept={vi.fn()} onDecline={onDecline} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onDecline).toHaveBeenCalled();
  });

  it('calls onDecline when clicking the backdrop', () => {
    const onDecline = vi.fn();
    const { container } = render(<TelemetryConsentModal onAccept={vi.fn()} onDecline={onDecline} />);
    fireEvent.click(container.firstChild as Element);
    expect(onDecline).toHaveBeenCalled();
  });

  it('does not call onDecline when clicking inside the modal card', () => {
    const onDecline = vi.fn();
    render(<TelemetryConsentModal onAccept={vi.fn()} onDecline={onDecline} />);
    fireEvent.click(screen.getByText('Help improve Postlane'));
    expect(onDecline).not.toHaveBeenCalled();
  });

  it('calls onAccept when Accept button is clicked', () => {
    const onAccept = vi.fn();
    render(<TelemetryConsentModal onAccept={onAccept} onDecline={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /yes, send anonymous data/i }));
    expect(onAccept).toHaveBeenCalled();
  });

  it('renders a link to the privacy policy', () => {
    render(<TelemetryConsentModal onAccept={vi.fn()} onDecline={vi.fn()} />);
    const link = screen.getByRole('link', { name: /privacy policy/i });
    expect(link).toBeInTheDocument();
    expect(link).toHaveAttribute('href', expect.stringContaining('privacy'));
  });
});
