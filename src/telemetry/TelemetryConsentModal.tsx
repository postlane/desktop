// SPDX-License-Identifier: BUSL-1.1

import { Button } from '../components/catalyst/button';

interface Props {
  onAccept: () => void;
  onDecline: () => void;
}

export default function TelemetryConsentModal({ onAccept, onDecline }: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl bg-white p-6 shadow-xl dark:bg-zinc-900 space-y-4">
        <h2 className="text-base font-semibold text-zinc-900 dark:text-zinc-100">Help improve Postlane</h2>
        <p className="text-sm text-zinc-600 dark:text-zinc-400">
          Send anonymous usage data — which skills you use, whether posts are approved or dismissed,
          which scheduler you use. No post content. No repo names. No personal information.
          You can change this in Settings → App.
        </p>
        <div className="flex gap-3">
          <Button onClick={onAccept}>Yes, send anonymous data</Button>
          <Button plain onClick={onDecline}>No thanks</Button>
        </div>
      </div>
    </div>
  );
}
