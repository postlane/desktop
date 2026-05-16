// SPDX-License-Identifier: BUSL-1.1

import { useEffect, useRef } from 'react';
import { VoiceGuideSection } from './VoiceGuideSection';

interface Props {
  repoName: string;
  projectId?: string;
  onClose: () => void;
}

export default function RepoConfigureModal({ repoName, projectId, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    ref.current?.focus();
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <div className="modal is-active">
      <div className="modal-background" onClick={onClose} />
      <div className="modal-card" role="dialog" aria-modal="true" ref={ref} tabIndex={-1}>
        <header className="modal-card-head">
          <p className="modal-card-title">Configure {repoName}</p>
          <button className="delete" onClick={onClose} aria-label="Close" />
        </header>
        <section className="modal-card-body">
          {projectId && <VoiceGuideSection projectId={projectId} />}
        </section>
        <footer className="modal-card-foot is-justify-content-flex-end">
          <button className="button is-ghost" onClick={onClose}>Close</button>
        </footer>
      </div>
    </div>
  );
}
