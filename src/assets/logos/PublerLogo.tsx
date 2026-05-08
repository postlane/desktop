// SPDX-License-Identifier: BUSL-1.1

import type { CSSProperties } from 'react';

interface Props {
  size?: number;
  style?: CSSProperties;
}

export function PublerLogo({ size = 16, style }: Props) {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="currentColor" aria-hidden="true" style={style}>
      {/* TODO: replace with correct path from https://brandfetch.com/publer.com */}
      <path d="M6 4h12a8 8 0 010 16H10v8H6V4zm4 4v8h8a4 4 0 000-8h-8z" />
    </svg>
  );
}
