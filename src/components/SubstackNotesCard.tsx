// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { countSubstackNotesChars } from './charCount';
import CardActions from './CardActions';

const LIMIT = 300;

interface SubstackNotesCardProps {
  content?: string;
  imageUrl?: string;
  onSave?: (_newContent: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
}

export default function SubstackNotesCard({
  content = '',
  imageUrl,
  onSave,
  onImageClick,
  onApprove,
  approveLabel = 'Approve',
  onDelete,
}: SubstackNotesCardProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    el.style.height = `${el.scrollHeight}px`;
  }, [draft, editing]);

  const count = countSubstackNotesChars(editing ? draft : content);
  const isOverLimit = count > LIMIT;
  const counterClass = isOverLimit
    ? 'text-sm font-medium text-red-600 dark:text-red-400'
    : 'text-sm text-zinc-500 dark:text-zinc-400';

  return (
    <div className="flex flex-col gap-3">
      {editing ? (
        <textarea
          ref={textareaRef}
          className="w-full resize-none overflow-hidden rounded border border-zinc-300 p-2 text-sm text-zinc-900 dark:border-zinc-600 dark:bg-zinc-800 dark:text-zinc-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
          rows={1}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          autoFocus
        />
      ) : (
        <div className="whitespace-pre-wrap break-all text-sm text-zinc-900 dark:text-zinc-100">
          {content}
        </div>
      )}
      {imageUrl && !editing && (
        <img src={imageUrl} alt="Post image" className="w-full rounded-xl object-cover" style={{ aspectRatio: '16/9' }} />
      )}
      <CardActions
        editing={editing}
        count={count}
        limit={LIMIT}
        counterClass={counterClass}
        isOverLimit={isOverLimit}
        onSave={onSave}
        onImageClick={onImageClick}
        onApprove={onApprove}
        approveLabel={approveLabel}
        onDelete={onDelete}
        onCancelEdit={() => setEditing(false)}
        onSaveEdit={() => { onSave?.(draft); setEditing(false); }}
        onStartEdit={() => { setDraft(content); setEditing(true); }}
      />
    </div>
  );
}
