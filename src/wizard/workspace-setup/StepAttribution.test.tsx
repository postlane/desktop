// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';
import StepAttribution from './StepAttribution';

function renderStep(onNext = vi.fn(), onBack = vi.fn()) {
  render(
    <MantineProvider>
      <StepAttribution onNext={onNext} onBack={onBack} />
    </MantineProvider>,
  );
  return { onNext, onBack };
}

describe('StepAttribution', () => {
  it('defaults the toggle to on', () => {
    renderStep();
    expect(screen.getByRole('switch', { name: /built with postlane/i })).toBeChecked();
  });

  it('advances with attribution: true by default', () => {
    const { onNext } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith({ attribution: true });
  });

  it('advances with attribution: false when toggled off', () => {
    const { onNext } = renderStep();
    fireEvent.click(screen.getByRole('switch', { name: /built with postlane/i }));
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith({ attribution: false });
  });
});
