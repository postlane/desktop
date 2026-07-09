// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';
import StepScheduler from './StepScheduler';

function renderStep(onNext = vi.fn(), onBack = vi.fn()) {
  render(
    <MantineProvider>
      <StepScheduler onNext={onNext} onBack={onBack} />
    </MantineProvider>,
  );
  return { onNext, onBack };
}

describe('StepScheduler', () => {
  it('offers only Zernio as the scheduler provider', () => {
    renderStep();
    const select = screen.getByLabelText(/scheduler/i) as HTMLSelectElement;
    const optionLabels = Array.from(select.options).map((o) => o.text);
    expect(optionLabels).toEqual(['Zernio']);
    expect(select.value).toBe('zernio');
  });

  it('masks the API key field', () => {
    renderStep();
    expect(screen.getByLabelText(/api key/i)).toHaveAttribute('type', 'password');
  });

  it('does not advance when the API key is empty', () => {
    const { onNext } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(screen.getByText(/api key is required/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();
  });

  it('advances with the correct patch shape, profile ID optional', () => {
    const { onNext } = renderStep();
    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: 'zk_live_secret' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith({
      scheduler_provider: 'zernio',
      scheduler_api_key: 'zk_live_secret',
      scheduler_profile_id: null,
    });
  });

  it('includes a filled-in profile ID in the patch', () => {
    const { onNext } = renderStep();
    fireEvent.change(screen.getByLabelText(/api key/i), { target: { value: 'zk_live_secret' } });
    fireEvent.change(screen.getByLabelText(/profile id/i), { target: { value: 'profile-42' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith(
      expect.objectContaining({ scheduler_profile_id: 'profile-42' }),
    );
  });
});
