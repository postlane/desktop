// SPDX-License-Identifier: BUSL-1.1

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
    <div className="is-flex is-align-items-center" style={{ gap: '0.5rem' }}>
      <span className={counterClass}>{count}/{limit}</span>
      <div className="is-flex is-align-items-center ml-auto" style={{ gap: '0.5rem' }}>
        {editing ? (
          <>
            <button className="button is-ghost is-small" onClick={onCancelEdit}>Cancel</button>
            <button className="button is-small" onClick={onSaveEdit} disabled={isOverLimit}>Save</button>
          </>
        ) : (
          <>
            {onSave && <button className="button is-ghost is-small" aria-label="Edit" onClick={onStartEdit}>Edit</button>}
            {onImageClick && <button className="button is-ghost is-small" aria-label="Image" onClick={onImageClick}>Image</button>}
            {onApprove && (
              <button className="button is-success is-small" onClick={onApprove} disabled={isOverLimit}>
                {approveLabel}
              </button>
            )}
            {onDelete && <button className="button is-danger is-small" onClick={onDelete}>Delete</button>}
          </>
        )}
      </div>
    </div>
  );
}
