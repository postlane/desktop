// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { UploadPostLogo } from './UploadPostLogo';

describe('UploadPostLogo', () => {
  it('renders an img element', () => {
    const { container } = render(<UploadPostLogo />);
    expect(container.querySelector('img')).toBeTruthy();
  });

  it('uses default size of 16', () => {
    const { container } = render(<UploadPostLogo />);
    const img = container.querySelector('img');
    expect(img?.getAttribute('width')).toBe('16');
  });

  it('uses custom size prop', () => {
    const { container } = render(<UploadPostLogo size={48} />);
    const img = container.querySelector('img');
    expect(img?.getAttribute('width')).toBe('48');
  });

  it('passes style prop to img', () => {
    const { container } = render(<UploadPostLogo style={{ borderRadius: '4px' }} />);
    const img = container.querySelector('img') as HTMLImageElement;
    expect(img.style.borderRadius).toBe('4px');
  });

  it('hides the img on error', () => {
    const { container } = render(<UploadPostLogo />);
    const img = container.querySelector('img') as HTMLImageElement;
    fireEvent.error(img);
    expect(img.style.display).toBe('none');
  });
});
