// SPDX-License-Identifier: BUSL-1.1

import { invoke } from '../ipc/invoke';
import WizardShell from './WizardShell';

interface Props {
  schedulerLinked: boolean;
  onComplete: () => void;
  onBack: () => void;
}

export default function ModalComplete({ schedulerLinked, onComplete, onBack }: Props) {
  async function handleContinue() {
    try { await invoke('set_wizard_completed'); } catch (e) { console.warn('[wizard] set_wizard_completed failed:', e); }
    onComplete();
  }

  return (
    <WizardShell
      step={7}
      totalSteps={7}
      title="You're all set"
      subtitle="Your workspace is ready. Add repos from the dashboard to start drafting."
      onNext={handleContinue}
      nextLabel="Continue"
      onBack={onBack}
    >
      <div>
        {schedulerLinked && (
          <p className="mb-3">
            <span className="tag is-success is-light mr-2">&#10003;</span>
            Scheduler connected
          </p>
        )}
        <p className="is-size-7 has-text-grey">
          Add a repo, then run <code>npx @postlane/cli draft-post</code> in a terminal inside that repo to draft your first post.
        </p>
      </div>
    </WizardShell>
  );
}
