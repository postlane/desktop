// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MantineProvider } from '@mantine/core';
import '@testing-library/jest-dom';
import StepLlm from './StepLlm';

function renderStep(onNext = vi.fn(), onBack = vi.fn()) {
  render(
    <MantineProvider>
      <StepLlm onNext={onNext} onBack={onBack} />
    </MantineProvider>,
  );
  return { onNext, onBack };
}

describe('StepLlm', () => {
  it('defaults to anthropic with a prefilled curated model', () => {
    renderStep();
    expect(screen.getByDisplayValue('Anthropic')).toBeInTheDocument();
    expect(screen.getByLabelText(/model/i)).toHaveValue('claude-sonnet-4-6');
  });

  it('shows the data-disclosure notice with the selected provider label', () => {
    renderStep();
    expect(screen.getByText(/post drafts and recent git context will be sent to anthropic/i)).toBeInTheDocument();
  });

  it('updates the disclosure notice and prefilled model when the provider changes', () => {
    renderStep();
    fireEvent.change(screen.getByLabelText(/provider/i), { target: { value: 'openai' } });
    expect(screen.getByText(/post drafts and recent git context will be sent to openai/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/model/i)).toHaveValue('gpt-4o');
  });

  it('allows free-text model entry for a provider with no curated list', () => {
    renderStep();
    fireEvent.change(screen.getByLabelText(/provider/i), { target: { value: 'mistral' } });
    expect(screen.getByLabelText(/model/i)).toHaveValue('');
    fireEvent.change(screen.getByLabelText(/model/i), { target: { value: 'mistral-large-latest' } });
    expect(screen.getByLabelText(/model/i)).toHaveValue('mistral-large-latest');
  });

  it('advances with the correct patch shape on submit', () => {
    const { onNext } = renderStep();
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledWith({ llm_provider: 'anthropic', llm_model: 'claude-sonnet-4-6' });
  });

  it('does not advance when the model field is empty', () => {
    const { onNext } = renderStep();
    fireEvent.change(screen.getByLabelText(/provider/i), { target: { value: 'mistral' } });
    fireEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(screen.getByText(/model is required/i)).toBeInTheDocument();
    expect(onNext).not.toHaveBeenCalled();
  });
});
