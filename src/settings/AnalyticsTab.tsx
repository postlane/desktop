// SPDX-License-Identifier: BUSL-1.1
// v2 stub — per-post analytics deferred; not wired to any route in M19

import { useState, useEffect } from 'react';
import { invoke } from '../ipc/invoke';

interface Props {
  repoId: string | null;
}

function ScriptTag({ token }: { token: string }) {
  const tag = `<script src="https://cdn.postlane.dev/p.js" data-site="${token}" defer></script>`;
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(tag);
      setCopied(true);
      setCopyError(null);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopyError('Failed to copy — please select and copy the text manually.');
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
      <p className="is-size-7 has-text-grey">Add this tag to the <code>&lt;head&gt;</code> of your site.</p>
      <pre className="is-size-7 has-background-light p-3" style={{ overflowX: 'auto', borderRadius: '0.375rem', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>{tag}</pre>
      <div className="is-flex is-align-items-center" style={{ gap: '0.75rem' }}>
        <button className="button is-outlined is-small" onClick={handleCopy}>{copied ? 'Copied!' : 'Copy'}</button>
        {copyError && <span className="is-size-7 has-text-grey">{copyError}</span>}
      </div>
      <a href="https://postlane.dev/docs/analytics" target="_blank" rel="noreferrer" className="is-size-7 has-text-link">How attribution works →</a>
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
      .catch(() => setError('not-signed-in'))
      .finally(() => setLoading(false));
  }, [repoId]);

  return { siteToken, loading, error };
}

export default function AnalyticsTab({ repoId }: Props) {
  const { siteToken, loading, error } = useAnalyticsTab(repoId);

  if (!repoId) {
    return <p className="is-size-7 has-text-grey">Select a repo to view its analytics snippet.</p>;
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
      <h2 className="has-text-weight-semibold is-size-7">Postlane Analytics</h2>
      <p className="is-size-7 has-text-grey">
        Track sessions arriving from your Postlane-scheduled posts. The snippet fires only when
        <code> utm_source=postlane</code> is present — no cookies, no PII.
      </p>
      {loading && <p className="is-size-7 has-text-grey">Loading…</p>}
      {error && (
        <p className="is-size-7 has-text-grey">
          Sign in at{' '}
          <a href="https://postlane.dev" target="_blank" rel="noreferrer" className="has-text-link">
            postlane.dev
          </a>{' '}
          to enable analytics.
        </p>
      )}
      {siteToken && <ScriptTag token={siteToken} />}
    </div>
  );
}
