// SPDX-License-Identifier: BUSL-1.1

import { useState, useEffect, useRef } from 'react';
import { Button } from './catalyst/button';
import { countCharsX } from './charCount';

const LIMIT = 280;
const AMBER_THRESHOLD = 250;

interface XCardProps {
  content?: string;
  imageUrl?: string;
  onSave?: (newContent: string) => void;
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

  const counterClass = isOverLimit
    ? 'text-sm font-medium text-red-600 dark:text-red-400'
    : isAmber
      ? 'text-sm font-medium text-amber-600 dark:text-amber-400'
      : 'text-sm text-zinc-500 dark:text-zinc-400';

  function startEditing() {
    setDraft(content);
    setEditing(true);
  }

  function handleSave() {
    onSave?.(draft);
    setEditing(false);
  }

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
        <img
          src={imageUrl}
          alt="Post image"
          className="w-full rounded-xl object-cover"
          style={{ aspectRatio: '16/9' }}
        />
      )}
      <div className="flex items-center gap-2">
        <span className={counterClass}>{count}/{LIMIT}</span>
        <div className="ml-auto flex items-center gap-2">
          {editing ? (
            <>
              <Button plain onClick={() => setEditing(false)}>Cancel</Button>
              <Button color="zinc" onClick={handleSave} disabled={isOverLimit}>Save</Button>
            </>
          ) : (
            <>
              {onSave && <Button plain onClick={startEditing} aria-label="Edit">Edit</Button>}
              {onImageClick && <Button plain onClick={onImageClick} aria-label="Image">Image</Button>}
              {onApprove && (
                <Button color="green" onClick={onApprove} disabled={isOverLimit}>
                  {approveLabel}
                </Button>
              )}
              {onDelete && (
                <Button color="rose" onClick={onDelete}>Delete</Button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
