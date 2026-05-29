// SPDX-License-Identifier: BUSL-1.1

import { useState } from 'react';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';

interface Props {
  workspacePath: string;
  onDismiss?: () => void;
}

interface Variant {
  label: string;
  file: string;
  snippet: string;
}

function buildVariants(workspacePath: string): Variant[] {
  const guidePath = `${workspacePath}/voice_guide.md`;
  return [
    { label: 'Claude Code', file: 'CLAUDE.md', snippet: `@${guidePath}` },
    { label: 'Cursor', file: '.cursorrules', snippet: guidePath },
    { label: 'Generic', file: 'context file', snippet: guidePath },
  ];
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  async function handleCopy() {
    try { await writeText(text); setCopied(true); setTimeout(() => setCopied(false), 1500); }
    catch { /* clipboard unavailable */ }
  }
  return (
    <button className="button is-small is-ghost" onClick={handleCopy} aria-label="Copy snippet">
      {copied ? '✓' : 'Copy'}
    </button>
  );
}

function SnippetRow({ variant }: { variant: Variant }) {
  return (
    <div className="mb-2">
      <p className="is-size-7 has-text-grey mb-1">
        {variant.label} — add to <code>{variant.file}</code>:
      </p>
      <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
        <code className="is-size-7" style={{
          background: 'var(--bulma-scheme-main-ter)',
          padding: '0.25rem 0.5rem',
          borderRadius: 4,
          flexGrow: 1,
          wordBreak: 'break-all',
        }}>
          {variant.snippet}
        </code>
        <CopyButton text={variant.snippet} />
      </div>
    </div>
  );
}

export default function VoiceGuideHint({ workspacePath, onDismiss }: Props) {
  const variants = buildVariants(workspacePath);
  return (
    <div className="notification is-success is-light mt-3" style={{ position: 'relative' }}>
      {onDismiss && <button className="delete" onClick={onDismiss} aria-label="Dismiss voice guide hint" />}
      <p className="is-size-7 has-text-weight-medium mb-2">Workspace ready — add voice guide to your IDE</p>
      <p className="is-size-7 has-text-grey mb-3">
        The voice guide will appear at <code>{workspacePath}/voice_guide.md</code> after
        your first save. Add a reference to your IDE&apos;s context file:
      </p>
      {variants.map((v) => <SnippetRow key={v.label} variant={v} />)}
    </div>
  );
}
