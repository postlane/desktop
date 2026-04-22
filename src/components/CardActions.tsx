// SPDX-License-Identifier: BUSL-1.1

import { Button } from './catalyst/button';

interface CardActionsProps {
  editing: boolean;
  count: number;
  limit: number;
  counterClass: string;
  isOverLimit: boolean;
  onSave?: (_content: string) => void;
  onImageClick?: () => void;
  onApprove?: () => void;
  approveLabel?: string;
  onDelete?: () => void;
  onCancelEdit: () => void;
  onSaveEdit: () => void;
  onStartEdit: () => void;
}

export default function CardActions({
  editing,
  count,
  limit,
  counterClass,
  isOverLimit,
  onSave,
  onImageClick,
  onApprove,
  approveLabel = 'Approve',
  onDelete,
  onCancelEdit,
  onSaveEdit,
  onStartEdit,
}: CardActionsProps) {
  return (
    <div className="flex items-center gap-2">
      <span className={counterClass}>{count}/{limit}</span>
      <div className="ml-auto flex items-center gap-2">
        {editing ? (
          <>
            <Button plain onClick={onCancelEdit}>Cancel</Button>
            <Button color="zinc" onClick={onSaveEdit} disabled={isOverLimit}>Save</Button>
          </>
        ) : (
          <>
            {onSave && <Button plain onClick={onStartEdit} aria-label="Edit">Edit</Button>}
            {onImageClick && <Button plain onClick={onImageClick} aria-label="Image">Image</Button>}
            {onApprove && (
              <Button color="green" onClick={onApprove} disabled={isOverLimit}>
                {approveLabel}
              </Button>
            )}
            {onDelete && <Button color="rose" onClick={onDelete}>Delete</Button>}
          </>
        )}
      </div>
    </div>
  );
}
