// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import type { ReactNode, RefObject } from 'react';
import { countCharsMastodon } from './charCount';
import CardActions from './CardActions';

const DEFAULT_LIMIT = 500;
const CW_PREFIX = 'CW:';

interface MastodonCardProps {
  content?: string;
  imageUrl?: string;
  charLimit?: number;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

function parseMastodonHTML(input: string): ReactNode {
  const nodes: ReactNode[] = [];
  const regex = /<(b|i|a)(?:\s+href="([^"]*)")?>(.*?)<\/\1>/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = regex.exec(input)) !== null) {
    if (match.index > lastIndex) nodes.push(input.slice(lastIndex, match.index));
    const tag = match[1];
    const href = match[2];
    const inner = match[3] ?? '';
    if (tag === 'b') {
      nodes.push(<strong key={match.index}>{inner}</strong>);
    } else if (tag === 'i') {
      nodes.push(<em key={match.index}>{inner}</em>);
    } else if (tag === 'a' && href !== undefined && /^https:\/\//.test(href)) {
      nodes.push(<a key={match.index} href={href} rel="noopener noreferrer" target="_blank">{inner}</a>);
    }
    lastIndex = regex.lastIndex;
  }
  if (lastIndex < input.length) nodes.push(input.slice(lastIndex));
  if (nodes.length === 0) return null;
  if (nodes.length === 1) return nodes[0];
  return <>{nodes}</>;
}

interface MastodonDisplayProps {
  editing: boolean;
  draft: string;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  hasCW: boolean;
  cwText: string | null;
  bodyContent: string;
  content: string;
  onDraftChange: (_v: string) => void;
}

function MastodonDisplay({ editing, draft, textareaRef, hasCW, cwText, bodyContent, content, onDraftChange }: MastodonDisplayProps) {
  return (
    <>
      {!editing && cwText !== null && (
        <div data-testid="cw-bar" className="notification is-warning is-light py-1 px-3 is-size-7">
          CW: {cwText}
        </div>
      )}
      {editing ? (
        <textarea ref={textareaRef} rows={1} value={draft} autoFocus onChange={(e) => onDraftChange(e.target.value)}
          className="textarea is-small"
        />
      ) : (
        <div data-testid="mastodon-body" className="content is-small">
          {parseMastodonHTML(hasCW ? bodyContent : content)}
        </div>
      )}
    </>
  );
}

export default function MastodonCard({ content = '', imageUrl, charLimit = DEFAULT_LIMIT, onSave, onImageClick, onApprove, approveLabel = 'Approve', onDelete }: MastodonCardProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [draft, editing]);

  const displayed = editing ? draft : content;
  const hasCW = content.startsWith(CW_PREFIX);
  const cwText = hasCW ? content.slice(CW_PREFIX.length).trim().split('\n')[0] : null;
  const bodyContent = hasCW ? content.slice(content.indexOf('\n') + 1) : content;
  const count = countCharsMastodon(displayed);
  const isOverLimit = count > charLimit;
  const amberThreshold = charLimit - 50;
  const cc = isOverLimit ? 'is-size-7 has-text-danger' : count >= amberThreshold ? 'is-size-7 has-text-warning-dark' : 'is-size-7 has-text-grey';

  return (
    <div className="block">
      <MastodonDisplay editing={editing} draft={draft} textareaRef={textareaRef}
        hasCW={hasCW} cwText={cwText} bodyContent={bodyContent} content={content}
        onDraftChange={setDraft}
      />
      {imageUrl && !editing && (
        <img src={imageUrl} alt="Post image" style={{ width: '100%', aspectRatio: '16/9', objectFit: 'cover' }} />
      )}
      <CardActions editing={editing} count={count} limit={charLimit} counterClass={cc}
        isOverLimit={isOverLimit} onSave={onSave} onImageClick={onImageClick}
        onApprove={onApprove} approveLabel={approveLabel} onDelete={onDelete}
        onCancelEdit={() => setEditing(false)}
        onSaveEdit={() => { onSave?.(draft); setEditing(false); }}
        onStartEdit={() => { setDraft(content); setEditing(true); }}
      />
    </div>
  );
}
