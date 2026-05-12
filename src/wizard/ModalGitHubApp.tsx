// SPDX-License-Identifier: BUSL-1.1

import { openUrl } from '@tauri-apps/plugin-opener';
import WizardShell from './WizardShell';

const GITHUB_APP_INSTALL_URL = 'https://github.com/apps/postlane/installations/new';
const CLI_COMMAND = 'npx @postlane/cli init';

interface Props {
  provider: string;
  onNext: () => void;
  onBack: () => void;
}

function GitHubInstallContent() {
  function handleInstall() {
    openUrl(GITHUB_APP_INSTALL_URL).catch(console.error);
  }

  return (
    <div>
      <p className="is-size-7 mb-4">
        Installing the Postlane GitHub App connects all repos in your org automatically.
        No per-repo CLI setup required.
      </p>
      <button className="button is-primary" onClick={handleInstall}>
        Install GitHub App
      </button>
      <p className="is-size-7 has-text-grey mt-3">
        After installing, Postlane will detect your repos automatically. Use Skip to connect repos
        manually from the dashboard instead.
      </p>
    </div>
  );
}

function CliContent() {
  return (
    <div>
      <p className="is-size-7 mb-3">
        Run the following CLI command inside each repo you want to connect:
      </p>
      <code className="is-family-code is-size-7 px-2 py-1 has-background-light" style={{ display: 'block' }}>
        {CLI_COMMAND}
      </code>
      <p className="is-size-7 has-text-grey mt-3">
        This window updates automatically when a repo is detected.
      </p>
    </div>
  );
}

export default function ModalGitHubApp({ provider, onNext, onBack }: Props) {
  const isGitHub = provider === 'github';

  return (
    <WizardShell
      step={5}
      totalSteps={6}
      title="Connect repos"
      subtitle={
        isGitHub
          ? 'Install the GitHub App to connect your org\'s repos.'
          : 'Connect your repos with the CLI.'
      }
      onNext={onNext}
      onBack={onBack}
      nextHidden={isGitHub}
      onSkip={isGitHub ? onNext : undefined}
    >
      {isGitHub ? <GitHubInstallContent /> : <CliContent />}
    </WizardShell>
  );
}
