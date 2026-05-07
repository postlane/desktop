// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import { countCharsBluesky } from './charCount';
import CardActions from './CardActions';

const LIMIT = 300;

interface BlueskyCardProps {
  content?: string;
  imageUrl?: string;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

function parseMarkdown(text: string): ReactNode[] {
  const segments = text.split(/(https?:\/\/[^\s]+)/g);
  const result: ReactNode[] = [];
  segments.forEach((segment, i) => {
    if (/^https?:\/\//.test(segment)) { result.push(segment); return; }
    segment.split(/(\*\*[^*]+\*\*|_[^_]+_)/).forEach((part, j) => {
      if (part.startsWith('**') && part.endsWith('**') && part.length > 4) {
        result.push(<strong key={`${i}-${j}`}>{part.slice(2, -2)}</strong>);
      } else if (part.startsWith('_') && part.endsWith('_') && part.length > 2) {
        result.push(<em key={`${i}-${j}`}>{part.slice(1, -1)}</em>);
      } else {
        result.push(part);
      }
    });
  });
  return result;
}

export default function BlueskyCard({
  content = '',
  imageUrl,
  onSave,
  onImageClick,
  onApprove,
  approveLabel = 'Approve',
  onDelete,
}: BlueskyCardProps) {
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
  const count = countCharsBluesky(displayed);
  const isOverLimit = count > LIMIT;
  const cc = isOverLimit ? 'is-size-7 has-text-danger' : 'is-size-7 has-text-grey';

  return (
    <div className="block">
      {editing ? (
        <textarea ref={textareaRef} className="textarea is-small" rows={1} value={draft}
          onChange={(e) => setDraft(e.target.value)} autoFocus />
      ) : (
        <div className="content is-small">{parseMarkdown(content)}</div>
      )}
      {imageUrl && !editing && (
        <img src={imageUrl} alt="Post image" style={{ width: '100%', aspectRatio: '16/9', objectFit: 'cover' }} />
      )}
      <CardActions
        editing={editing} count={count} limit={LIMIT} counterClass={cc}
        isOverLimit={isOverLimit} onSave={onSave} onImageClick={onImageClick}
        onApprove={onApprove} approveLabel={approveLabel} onDelete={onDelete}
        onCancelEdit={() => setEditing(false)}
        onSaveEdit={() => { onSave?.(draft); setEditing(false); }}
        onStartEdit={() => { setDraft(content); setEditing(true); }}
      />
    </div>
  );
}
