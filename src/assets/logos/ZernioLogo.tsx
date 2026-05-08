// SPDX-License-Identifier: BUSL-1.1

import type { CSSProperties } from 'react';

interface Props {
  size?: number;
  style?: CSSProperties;
}

export function ZernioLogo({ size = 16, style }: Props) {
  return (
    <svg width={size} height={size} viewBox="0 170 620 645" fill="none" aria-hidden="true" style={style}>
      <path d="M-6.10352e-05 230.458L-0.000244141 461.78L359.783 367.324C404.618 355.553 445.318 396.864 432.88 441.519L332.554 801.71L561.96 801.71L609.501 568.36C613.469 548.882 607.541 528.703 593.669 514.466L287.32 200.054C273.062 185.42 252.341 179.041 232.321 183.12L-6.10352e-05 230.458Z" fill="#EB3514" />
    </svg>
  );
}
