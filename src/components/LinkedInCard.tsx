// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { countLinkedInChars } from './charCount';
import CardActions from './CardActions';

const LIMIT = 3000;

interface LinkedInCardProps {
  content?: string;
  imageUrl?: string;
  authorName?: string;
  authorHandle?: string;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

interface ReadViewProps {
  content: string;
  authorName?: string;
  authorHandle?: string;
}

function LinkedInReadView({ content, authorName, authorHandle }: ReadViewProps) {
  return (
    <div className="is-flex" style={{ gap: '0.5rem' }}>
      <div data-testid="avatar" className="image is-48x48 is-flex-shrink-0"
        style={{ background: 'var(--bulma-grey-lighter)', borderRadius: '4px' }} />
      <div>
        {authorName && <p className="has-text-weight-semibold is-size-7">{authorName}</p>}
        {authorHandle && <p className="has-text-grey is-size-7">{authorHandle} · 1st</p>}
        <div className="content is-small">{content}</div>
      </div>
    </div>
  );
}

export default function LinkedInCard({
  content = '',
  imageUrl,
  authorName,
  authorHandle,
  onSave,
  onImageClick,
  onApprove,
  approveLabel = 'Approve',
  onDelete,
}: LinkedInCardProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [draft, editing]);

  const count = countLinkedInChars(editing ? draft : content);
  const isOverLimit = count > LIMIT;
  const cc = isOverLimit ? 'is-size-7 has-text-danger' : 'is-size-7 has-text-grey';

  return (
    <div className="block">
      {editing ? (
        <textarea ref={textareaRef} className="textarea is-small" rows={1} value={draft}
          onChange={(e) => setDraft(e.target.value)} autoFocus />
      ) : (
        <LinkedInReadView content={content} authorName={authorName} authorHandle={authorHandle} />
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
