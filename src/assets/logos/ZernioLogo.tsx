// SPDX-License-Identifier: BUSL-1.1

import type { CSSProperties } from 'react';

interface Props {
  size?: number;
  style?: CSSProperties;
}

export function ZernioLogo({ size = 16, style }: Props) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="currentColor" aria-hidden="true" style={style}>
      {/* TODO: replace with correct path from https://brandfetch.com/zernio.com */}
      <path d="M4 6h24v4L10 22h18v4H4v-4L22 10H4V6z" />
    </svg>
  );
}
