// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { countCharsX } from './charCount';
import CardActions from './CardActions';

const LIMIT = 280;
const AMBER_THRESHOLD = 250;

interface XCardProps {
  content?: string;
  imageUrl?: string;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

export default function XCard({
  content = '',
  imageUrl,
  onSave,
  onImageClick,
  onApprove,
  approveLabel = 'Approve',
  onDelete,
}: XCardProps) {
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
  const count = countCharsX(displayed);
  const isOverLimit = count > LIMIT;
  const isAmber = count > AMBER_THRESHOLD && !isOverLimit;
  const cc = isOverLimit ? 'is-size-7 has-text-danger' : isAmber ? 'is-size-7 has-text-warning-dark' : 'is-size-7 has-text-grey';

  return (
    <div className="block">
      {editing ? (
        <textarea ref={textareaRef} className="textarea is-small" rows={1} value={draft}
          onChange={(e) => setDraft(e.target.value)} autoFocus />
      ) : (
        <div className="content is-small">{content}</div>
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
