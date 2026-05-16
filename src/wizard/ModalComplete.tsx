// SPDX-License-Identifier: BUSL-1.1

import { invoke } from '../ipc/invoke';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

const DOCS_URL = 'https://docs.postlane.dev';

interface Props {
  schedulerLinked: boolean;
  repoConnected: boolean;
  onComplete: () => void;
  onBack: () => void;
}

export default function ModalComplete({ schedulerLinked, repoConnected, onComplete, onBack }: Props) {
  async function handleContinue() {
    invoke('clear_wizard_state').catch(console.warn);
    try { await invoke('set_wizard_completed'); } catch (e) { console.warn('[wizard] set_wizard_completed failed:', e); }
    onComplete();
  }

  const subtitle = repoConnected
    ? 'Your workspace is ready to start drafting.'
    : 'Your workspace is ready. Add repos from the dashboard to start drafting.';

  return (
    <WizardShell
      step={7}
      totalSteps={7}
      title="You're all set"
      subtitle={subtitle}
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
        {repoConnected ? (
          <p className="is-size-7 has-text-grey">
            Run <code>/draft-post</code> in a terminal inside that repo to draft your first post.{' '}
            <a href="#" onClick={(e) => { e.preventDefault(); openUrl(DOCS_URL).catch(console.warn); }}>
              Documentation
            </a>
          </p>
        ) : (
          <p className="is-size-7 has-text-grey">
            Add a repo, then run <code>/draft-post</code> in a terminal inside that repo to draft your first post.
          </p>
        )}
      </div>
    </WizardShell>
  );
}
