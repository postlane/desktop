// SPDX-License-Identifier: BUSL-1.1

import { GitHubLogo, GitLabLogo } from '../assets/logos';
import WizardShell from './WizardShell';

interface Props {
  currentProvider: string;
  linkedProviders: string[];
  onContinue: () => void;
}

function providerLabel(provider: string): string {
  return provider === 'gitlab' ? 'GitLab' : 'GitHub';
}

function ProviderBadge({ provider }: { provider: string }) {
  const Logo = provider === 'gitlab' ? GitLabLogo : GitHubLogo;
  return (
    <span className="is-flex is-align-items-center" style={{ gap: 6 }}>
      <Logo size={14} />
      <span>{providerLabel(provider)}</span>
    </span>
  );
}

export default function ModalProviderLinked({ currentProvider, linkedProviders, onContinue }: Props) {
  const existingProviders = linkedProviders.filter((p) => p !== currentProvider);

  return (
    <WizardShell
      step={3}
      totalSteps={3}
      title="Account already exists"
      subtitle="We found an existing Postlane account linked to this email address."
      onNext={onContinue}
      onBack={onContinue}
      nextLabel="Got it, continue"
      nextHidden={false}
    >
      <div className="content is-size-6">
        <p>
          Your <strong>{providerLabel(currentProvider)}</strong> account has been added to your existing
          Postlane account. Your workspaces, billing, and settings are shared across all linked providers.
        </p>
        <div className="mt-4">
          <p className="is-size-7 has-text-grey mb-2">Linked accounts:</p>
          <div className="is-flex" style={{ gap: 12, flexWrap: 'wrap' }}>
            {existingProviders.map((p) => (
              <span key={p} className="tag is-light is-medium">
                <ProviderBadge provider={p} />
              </span>
            ))}
            <span className="tag is-primary is-medium">
              <ProviderBadge provider={currentProvider} />
              <span className="ml-2 is-size-7">just added</span>
            </span>
          </div>
        </div>
      </div>
    </WizardShell>
  );
}
