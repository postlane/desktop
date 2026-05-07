// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react';

interface Props {
  onAccept: () => void;
  onDecline: () => void;
}

export default function TelemetryConsentModal({ onAccept, onDecline }: Props) {
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onDecline(); };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [onDecline]);

  return (
    <div className="modal is-active" onClick={onDecline}>
      <div className="modal-background" />
      <div className="modal-card" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true">
        <header className="modal-card-head">
          <p className="modal-card-title is-size-6">Help improve Postlane</p>
        </header>
        <section className="modal-card-body">
          <p className="is-size-7 has-text-grey mb-3">
            Send anonymous usage data — which skills you use, whether posts are approved or dismissed,
            which scheduler you use. No post content. No repo names. No personal information.
            You can change this in Settings &rarr; App.
          </p>
          <a href="https://postlane.dev/docs/privacy" target="_blank" rel="noreferrer"
            className="is-size-7 has-text-link">Privacy policy &rarr;</a>
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end" style={{ gap: '0.5rem' }}>
          <button className="button is-ghost" onClick={onDecline}>No thanks</button>
          <button className="button is-primary" onClick={onAccept}>Yes, send anonymous data</button>
        </footer>
      </div>
    </div>
  );
}
