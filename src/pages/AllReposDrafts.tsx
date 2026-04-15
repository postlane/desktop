// SPDX-License-Identifier: BUSL-1.1

import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useState } from 'react';

interface Props {
  postWizardNudge: boolean;
  onNudgeDismissed: () => void;
}

export default function AllReposDrafts({ postWizardNudge, onNudgeDismissed }: Props) {
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'fallback'>('idle');

  async function handleCopy() {
    try {
      await writeText('/draft-post');
      setCopyState('copied');
      setTimeout(() => setCopyState('idle'), 2000);
    } catch {
      setCopyState('fallback');
    }
  }

  if (postWizardNudge) {
    return (
      <div className="flex h-full items-center justify-center p-8">
        <div className="max-w-sm text-center">
          <p className="mb-4 font-medium text-zinc-900 dark:text-zinc-100">
            You're set up.
          </p>
          <p className="mb-6 text-sm text-zinc-600 dark:text-zinc-400">
            Open your IDE in a registered repo and run:
          </p>
          <div className="mb-6 flex items-center justify-center gap-3 rounded-lg bg-zinc-100 px-4 py-3 dark:bg-zinc-800">
            <code className="font-mono text-sm text-zinc-900 dark:text-zinc-100">
              /draft-post
            </code>
            <button
              onClick={handleCopy}
              aria-label="Copy /draft-post command"
              className="rounded px-2 py-1 text-xs text-zinc-500 hover:bg-zinc-200 dark:hover:bg-zinc-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              {copyState === 'copied' ? '✓ Copied' : '📋 Copy'}
            </button>
          </div>
          {copyState === 'fallback' && (
            <p className="mb-4 text-xs text-zinc-500">Press Ctrl+C to copy</p>
          )}
          <p className="text-sm text-zinc-500">
            Your first draft will appear here when it's ready.
          </p>
          <button
            onClick={onNudgeDismissed}
            className="mt-6 text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
          >
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full items-center justify-center p-8">
      <p className="text-sm text-zinc-500">
        No drafts waiting. Invoke <code className="font-mono">/draft-post</code> in your IDE to create one.
      </p>
    </div>
  );
}
