// SPDX-License-Identifier: BUSL-1.1

import type { MouseEvent } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

interface Props {
  onNext: () => void;
}

export default function ModalWelcome({ onNext }: Props) {
  function handlePricingLink(e: MouseEvent) {
    e.preventDefault();
    openUrl('https://postlane.dev/pricing').catch(console.error);
  }

  return (
    <WizardShell
      step={1}
      totalSteps={7}
      title="Welcome to Postlane"
      subtitle="Ship code. Tell the world."
      onNext={onNext}
      nextLabel="Get started"
    >
      <ol className="mb-5" style={{ paddingLeft: '1.25rem' }}>
        <li className="mb-2">Create your account</li>
        <li className="mb-2">Set up a workspace</li>
        <li>Connect a repo</li>
      </ol>
      <p className="is-size-7 has-text-grey">
        Free for your first workspace. $5/month per workspace after that.{' '}
        <a
          href="https://postlane.dev/pricing"
          onClick={handlePricingLink}
          className="has-text-link"
        >
          See pricing
        </a>
      </p>
    </WizardShell>
  );
}
