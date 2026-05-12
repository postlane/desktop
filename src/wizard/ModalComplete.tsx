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
    try { await invoke('set_wizard_completed'); } catch { /* non-fatal */ }
    onComplete();
  }

  return (
    <WizardShell
      step={6}
      totalSteps={6}
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
          Run <code>npx @postlane/cli init</code> inside any repo to connect it, then use
          the <code>/draft-post</code> slash command to draft your first post.
        </p>
      </div>
    </WizardShell>
  );
}
