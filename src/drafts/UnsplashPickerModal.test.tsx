// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import UnsplashPickerModal, { type UnsplashPhoto } from './UnsplashPickerModal';

function makePhoto(id: string): UnsplashPhoto {
  return {
    id,
    description: `Photo ${id}`,
    urls: {
      raw: `https://images.unsplash.com/${id}`,
      full: `https://images.unsplash.com/${id}?w=2000`,
      regular: `https://images.unsplash.com/${id}?w=1080`,
      small: `https://images.unsplash.com/${id}?w=400`,
      thumb: `https://images.unsplash.com/${id}?w=200`,
    },
    links: { download_location: `https://api.unsplash.com/photos/${id}/download` },
    user: { name: `Photographer ${id}`, links: { html: `https://unsplash.com/@user${id}` } },
  };
}

const PHOTOS = Array.from({ length: 20 }, (_, i) => makePhoto(`p${i}`));

const defaultProps = {
  photos: PHOTOS,
  onSelect: vi.fn(),
  onClose: vi.fn(),
  onLoadMore: vi.fn(),
  loadingMore: false,
};

describe('UnsplashPickerModal', () => {
  it('renders as an active Bulma modal', () => {
    render(<UnsplashPickerModal {...defaultProps} />);
    expect(document.querySelector('.modal.is-active')).toBeInTheDocument();
  });

  it('renders a thumbnail for each photo', () => {
    render(<UnsplashPickerModal {...defaultProps} />);
    const imgs = screen.getAllByRole('img');
    expect(imgs).toHaveLength(20);
  });

  it('does not show photographer names', () => {
    render(<UnsplashPickerModal {...defaultProps} />);
    expect(screen.queryByText(/Photographer p0/i)).not.toBeInTheDocument();
  });

  it('calls onSelect with url, download_location, and attribution when a thumbnail is clicked', () => {
    const onSelect = vi.fn();
    render(<UnsplashPickerModal {...defaultProps} onSelect={onSelect} />);
    fireEvent.click(screen.getAllByRole('img')[0]);
    expect(onSelect).toHaveBeenCalledWith(
      PHOTOS[0].urls.regular,
      PHOTOS[0].links.download_location,
      { photographer_name: PHOTOS[0].user.name, photographer_url: PHOTOS[0].user.links.html },
    );
  });

  it('calls onClose when the modal background is clicked', () => {
    const onClose = vi.fn();
    render(<UnsplashPickerModal {...defaultProps} onClose={onClose} />);
    const bg = document.querySelector('.modal-background');
    if (!bg) throw new Error('.modal-background not found');
    fireEvent.click(bg);
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    render(<UnsplashPickerModal {...defaultProps} onClose={onClose} />);
    fireEvent.click(screen.getByRole('button', { name: /close/i }));
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onLoadMore when the Load more button is clicked', () => {
    const onLoadMore = vi.fn();
    render(<UnsplashPickerModal {...defaultProps} onLoadMore={onLoadMore} />);
    fireEvent.click(screen.getByRole('button', { name: /load more/i }));
    expect(onLoadMore).toHaveBeenCalled();
  });

  it('disables the Load more button while loadingMore is true', () => {
    render(<UnsplashPickerModal {...defaultProps} loadingMore />);
    expect(screen.getByRole('button', { name: /load more/i })).toBeDisabled();
  });
});
