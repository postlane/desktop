// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { PublerLogo } from './PublerLogo';

describe('PublerLogo', () => {
  it('renders an svg element', () => {
    const { container } = render(<PublerLogo />);
    expect(container.querySelector('svg')).toBeTruthy();
  });

  it('uses default size of 16', () => {
    const { container } = render(<PublerLogo />);
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('width')).toBe('16');
  });

  it('uses custom size prop', () => {
    const { container } = render(<PublerLogo size={32} />);
    const svg = container.querySelector('svg');
    expect(svg?.getAttribute('width')).toBe('32');
  });

  it('passes style prop to svg', () => {
    const { container } = render(<PublerLogo style={{ opacity: 0.5 }} />);
    const svg = container.querySelector('svg') as SVGElement;
    expect((svg as unknown as HTMLElement).style.opacity).toBe('0.5');
  });
});
