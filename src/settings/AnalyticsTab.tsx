// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../components/catalyst/button';

interface Props {
  repoId: string | null;
}

function ScriptTag({ token }: { token: string }) {
  const tag = `<script src="https://cdn.postlane.dev/p.js" data-site="${token}" defer></script>`;
  const [copied, setCopied] = useState(false);

  async function handleCopy() {
    await navigator.clipboard.writeText(tag);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  return (
    <div className="space-y-3">
      <p className="text-xs text-zinc-500 dark:text-zinc-400">Add this tag to the <code>&lt;head&gt;</code> of your site.</p>
      <pre className="overflow-x-auto rounded-lg bg-zinc-100 px-3 py-2 text-xs dark:bg-zinc-800 whitespace-pre-wrap break-all">{tag}</pre>
      <Button outline onClick={handleCopy}>{copied ? 'Copied!' : 'Copy'}</Button>
    </div>
  );
}

function useAnalyticsTab(repoId: string | null) {
  const [siteToken, setSiteToken] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!repoId) { setLoading(false); return; }
    setLoading(true);
    invoke<string>('get_site_token', { repoId })
      .then((t) => { setSiteToken(t); setError(null); })
      .catch(() => setError('Sign in at postlane.dev to enable analytics.'))
      .finally(() => setLoading(false));
  }, [repoId]);

  return { siteToken, loading, error };
}

export default function AnalyticsTab({ repoId }: Props) {
  const { siteToken, loading, error } = useAnalyticsTab(repoId);

  if (!repoId) {
    return <p className="text-sm text-zinc-500">Select a repo to view its analytics snippet.</p>;
  }

  return (
    <div className="space-y-4">
      <h2 className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Postlane Analytics</h2>
      <p className="text-xs text-zinc-500 dark:text-zinc-400">
        Track sessions arriving from your Postlane-scheduled posts. The snippet fires only when
        <code> utm_source=postlane</code> is present — no cookies, no PII.
      </p>
      {loading && <p className="text-xs text-zinc-400">Loading…</p>}
      {error && <p className="text-xs text-zinc-500">{error}</p>}
      {siteToken && <ScriptTag token={siteToken} />}
    </div>
  );
}
