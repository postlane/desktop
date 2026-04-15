// SPDX-License-Identifier: BUSL-1.1

export default function Settings({ onClose }: { onClose: () => void }) {
  return (
    <div>
      Settings - Placeholder{' '}
      <button onClick={onClose}>Close</button>
    </div>
  );
}
