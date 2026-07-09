// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';
import StepBasicConfig from './StepBasicConfig';

function renderStep(onNext = vi.fn(), onBack = vi.fn()) {
  render(
    <MantineProvider>
      <StepBasicConfig onNext={onNext} onBack={onBack} />
    </MantineProvider>,
  );
  return { onNext, onBack };
}

function fillRequiredFields() {
  fireEvent.change(screen.getByLabelText(/base url/i), { target: { value: 'https://postlane.dev' } });
  fireEvent.click(screen.getByLabelText('X'));
  fireEvent.change(screen.getByLabelText(/author name/i), { target: { value: 'Jordan Reyes' } });
  fireEvent.change(screen.getByLabelText(/writing style/i), { target: { value: 'Direct, no jargon' } });
}

describe('StepBasicConfig', () => {
  it('renders all 8 platform checkboxes', () => {
    renderStep();
    for (const label of ['X', 'Bluesky', 'Mastodon', 'LinkedIn', 'Substack Notes', 'Product Hunt', 'Show HN', 'Changelog']) {
      expect(screen.getByLabelText(label)).toBeInTheDocument();
    }
  });

  it('hides the Mastodon instance field until Mastodon is checked', () => {
    renderStep();
    expect(screen.queryByLabelText(/mastodon instance/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Mastodon'));
    expect(screen.getByLabelText(/mastodon instance/i)).toBeInTheDocument();
  });

  it('rejects a base URL that does not start with https:// and does not advance', () => {
    const { onNext } = renderStep();
    fillRequiredFields();
    fireEvent.change(screen.getByLabelText(/base url/i), { target: { value: 'http://postlane.dev' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(screen.getByText(/must start with https:\/\//i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('requires at least one platform selected', () => {
    const { onNext } = renderStep();
    fireEvent.change(screen.getByLabelText(/base url/i), { target: { value: 'https://postlane.dev' } });
    fireEvent.change(screen.getByLabelText(/author name/i), { target: { value: 'Jordan Reyes' } });
    fireEvent.change(screen.getByLabelText(/writing style/i), { target: { value: 'Direct' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(screen.getByText(/select at least one platform/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('advances with the correct patch shape on valid submit', () => {
    const { onNext } = renderStep();
    fillRequiredFields();
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith({
      base_url: 'https://postlane.dev',
      platforms: ['x'],
      mastodon_instance: null,
      author: 'Jordan Reyes',
      style: 'Direct, no jargon',
      utm_campaign: null,
    });
  });

  it('includes mastodon_instance when Mastodon is checked and filled in', () => {
    const { onNext } = renderStep();
    fillRequiredFields();
    fireEvent.click(screen.getByLabelText('Mastodon'));
    fireEvent.change(screen.getByLabelText(/mastodon instance/i), { target: { value: 'mastodon.social' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith(
      expect.objectContaining({ platforms: ['x', 'mastodon'], mastodon_instance: 'mastodon.social' }),
    );
  });
});
