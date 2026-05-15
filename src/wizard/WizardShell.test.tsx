// SPDX-License-Identifier: BUSL-1.1

import type { ComponentProps } from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import WizardShell from './WizardShell';

const noop = () => {};

function shell(overrides: Partial<ComponentProps<typeof WizardShell>> = {}) {
  return render(
    <WizardShell step={1} totalSteps={5} title="Test Title" subtitle="Test subtitle" onNext={noop} {...overrides}>
      <p>body content</p>
    </WizardShell>
  );
}

describe('WizardShell', () => {
  it('test_renders_title_and_subtitle', () => {
    shell();
    expect(screen.getByText('Test Title')).toBeDefined();
    expect(screen.getByText('Test subtitle')).toBeDefined();
  });

  it('test_renders_children', () => {
    shell();
    expect(screen.getByText('body content')).toBeDefined();
  });

  it('test_back_button_hidden_when_no_onBack', () => {
    shell();
    expect(screen.queryByRole('button', { name: /back/i })).toBeNull();
  });

  it('test_back_button_calls_onBack', async () => {
    const onBack = vi.fn();
    shell({ onBack });
    await userEvent.click(screen.getByRole('button', { name: /back/i }));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it('test_next_button_disabled_when_prop_set', () => {
    shell({ nextDisabled: true });
    const next = screen.getByRole('button', { name: /next/i });
    expect((next as HTMLButtonElement).disabled).toBe(true);
  });

  it('test_next_button_calls_onNext', async () => {
    const onNext = vi.fn();
    shell({ onNext });
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    expect(onNext).toHaveBeenCalledOnce();
  });

  it('test_skip_shown_only_when_onSkip_provided', () => {
    shell();
    expect(screen.queryByRole('button', { name: /skip/i })).toBeNull();
    shell({ onSkip: noop });
    expect(screen.getAllByRole('button', { name: /skip/i }).length).toBeGreaterThan(0);
  });

  it('test_step_counter_renders_correctly', () => {
    shell({ step: 3, totalSteps: 5 });
    expect(screen.getByText('3 / 5')).toBeDefined();
  });

  it('test_next_label_override', () => {
    shell({ nextLabel: 'Get started' });
    expect(screen.getByRole('button', { name: /get started/i })).toBeDefined();
  });

  it('test_next_button_hidden_when_nextHidden_set', () => {
    shell({ nextHidden: true });
    expect(screen.queryByRole('button', { name: /next/i })).toBeNull();
  });

  it('shows empty string for unknown step number in step name label', () => {
    shell({ step: 99, totalSteps: 100 });
    // STEP_NAMES[99] is undefined — the ?? '' branch returns empty string
    const stepLabel = document.querySelector('.is-size-7.has-text-grey-light');
    expect(stepLabel).not.toBeNull();
    if (stepLabel) {
      expect(stepLabel.textContent).toContain('Step 99');
      expect(stepLabel.textContent).toContain('/ 100');
    }
  });
});
