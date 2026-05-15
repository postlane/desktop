// SPDX-License-Identifier: BUSL-1.1

interface Props {
  step: number;
  onResume: () => void;
  onStartOver: () => void;
}

export default function WizardResumePrompt({ step, onResume, onStartOver }: Props) {
  return (
    <div className="is-flex is-align-items-center is-justify-content-center" style={{ height: '100vh' }}>
      <div className="box" style={{ maxWidth: '22rem', textAlign: 'center' }}>
        <p className="is-size-6 has-text-weight-medium mb-3">Resume setup?</p>
        <p className="is-size-7 has-text-grey mb-4">
          You were on step {step} of 7 last time. Pick up where you left off, or start from the beginning.
        </p>
        <div className="buttons is-centered">
          <button className="button is-primary is-small" onClick={onResume}>
            Resume from step {step}
          </button>
          <button className="button is-small" onClick={onStartOver}>
            Start over
          </button>
        </div>
      </div>
    </div>
  );
}
