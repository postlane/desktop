// SPDX-License-Identifier: BUSL-1.1

import type { ReactNode } from 'react';

const STEP_NAMES: Record<number, string> = {
  1: 'WELCOME',
  2: 'ACCOUNT',
  3: 'WORKSPACE',
  4: 'SCHEDULER',
  5: 'INSTALL',
  6: 'VOICE',
  7: 'COMPLETE',
};

interface Props {
  step: number;
  totalSteps: number;
  title: string;
  subtitle: string;
  children: ReactNode;
  onNext: () => void;
  nextLabel?: string;
  nextDisabled?: boolean;
  nextHidden?: boolean;
  onBack?: () => void;
  onSkip?: () => void;
  skipLabel?: string;
}

interface FooterProps {
  step: number;
  totalSteps: number;
  onBack?: () => void;
  onSkip?: () => void;
  skipLabel: string;
  onNext: () => void;
  nextLabel: string;
  nextDisabled: boolean;
  nextHidden: boolean;
}

function WizardFooter({ step, totalSteps, onBack, onSkip, skipLabel, onNext, nextLabel, nextDisabled, nextHidden }: FooterProps) {
  return (
    <div className="px-5 py-3 is-flex is-align-items-center" style={{ borderTop: '1px solid #eee' }}>
      <div style={{ flex: 1 }}>
        {onBack && (
          <button className="button is-light is-small" onClick={onBack}>
            ← Back
          </button>
        )}
      </div>
      <span className="is-size-7 has-text-grey">{step} / {totalSteps}</span>
      <div style={{ flex: 1, display: 'flex', justifyContent: 'flex-end', gap: '0.5rem' }}>
        {onSkip && (
          <button className="button is-light is-small has-background-warning-light" onClick={onSkip}>
            {skipLabel}
          </button>
        )}
        {!nextHidden && (
          <button
            className="button is-primary is-small"
            onClick={onNext}
            disabled={nextDisabled}
          >
            {nextLabel} →
          </button>
        )}
      </div>
    </div>
  );
}

export default function WizardShell({
  step, totalSteps, title, subtitle, children, onNext,
  nextLabel = 'Next', nextDisabled = false, nextHidden = false,
  onBack, onSkip, skipLabel = 'Skip',
}: Props) {
  return (
    <div style={{ position: 'fixed', inset: 0, zIndex: 9999, overflow: 'hidden' }} className="has-background-white">
      <div className="px-5 pt-5 pb-4">
        <p className="is-size-7 has-text-grey-light mb-1" style={{ textTransform: 'uppercase', letterSpacing: '0.1em' }}>
          Step {String(step).padStart(2, '0')} / {String(totalSteps).padStart(2, '0')} — {STEP_NAMES[step] ?? ''}
        </p>
        <h1 className="title mb-1">{title}</h1>
        <p className="subtitle has-text-grey mb-4">{subtitle}</p>
        {children}
      </div>
      <WizardFooter step={step} totalSteps={totalSteps} onBack={onBack} onSkip={onSkip} skipLabel={skipLabel}
        onNext={onNext} nextLabel={nextLabel} nextDisabled={nextDisabled} nextHidden={nextHidden} />
    </div>
  );
}
