// SPDX-License-Identifier: BUSL-1.1

import { useEffect } from 'react';

interface Props {
  platforms: string[];
  onClose: () => void;
  autoDismissMs?: number;
}

export function SendSuccessModal({ platforms, onClose, autoDismissMs = 2500 }: Props) {
  useEffect(() => {
    const id = setTimeout(onClose, autoDismissMs);
    return () => clearTimeout(id);
  }, [onClose, autoDismissMs]);

  const text = platforms.length > 0
    ? `Sent to ${platforms.map(p => p.toUpperCase()).join(', ')}`
    : 'Sent';

  return (
    <div className="modal is-active" role="dialog" aria-label="Post sent">
      <div
        className="modal-background"
        data-testid="modal-background"
        onClick={onClose}
      />
      <div className="modal-content">
        <div className="box has-text-centered py-5">
          <span className="icon is-large has-text-success mb-3" style={{ fontSize: '2rem' }}>
            ✓
          </span>
          <p
            role="status"
            className="has-text-success has-text-weight-semibold is-size-5 mt-2"
          >
            {text}
          </p>
        </div>
      </div>
    </div>
  );
}
