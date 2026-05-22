// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import PostCardImageInput from './PostCardImageInput';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

const PHOTO = {
  id: 'abc123',
  description: 'A test photo',
  urls: {
    raw: 'https://images.unsplash.com/raw',
    full: 'https://images.unsplash.com/full',
    regular: 'https://images.unsplash.com/regular',
    small: 'https://images.unsplash.com/small',
    thumb: 'https://images.unsplash.com/thumb',
  },
  links: { download_location: 'https://api.unsplash.com/photos/abc123/download' },
  user: { name: 'Jane Doe', links: { html: 'https://unsplash.com/@janedoe' } },
};

const defaultProps = {
  imageUrl: null,
  imageInput: '',
  fetchingOg: false,
  ogFetchError: null,
  hasUnsplashKey: true,
  onInputChange: vi.fn(),
  onSave: vi.fn(),
  onRemove: vi.fn(),
  onCancel: vi.fn(),
  onSelectUnsplash: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue([]);
});

// 21.8.2 — search thumbnail grid
describe('PostCardImageInput search mode', () => {
  it('calls search_unsplash when Search button is clicked', async () => {
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('search_unsplash', { query: 'nature' });
    });
  });

  it('displays photo thumbnails when results are returned', async () => {
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByAltText(/a test photo/i)).toBeInTheDocument();
  });

  it('shows photographer name below each thumbnail', async () => {
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByText(/jane doe/i)).toBeInTheDocument();
  });

  it('calls onSelectUnsplash with url, download_location, and attribution when thumbnail clicked', async () => {
    const onSelectUnsplash = vi.fn();
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} onSelectUnsplash={onSelectUnsplash} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    fireEvent.click(await screen.findByAltText(/a test photo/i));
    expect(onSelectUnsplash).toHaveBeenCalledWith(
      PHOTO.urls.regular,
      PHOTO.links.download_location,
      { photographer_name: PHOTO.user.name, photographer_url: PHOTO.user.links.html },
    );
  });

  // 21.8.23 — rate limit handling
  it('shows rate-limit message when search returns rate_limit error', async () => {
    mockInvoke.mockRejectedValue(new Error('rate_limit'));
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByText(/search limit reached/i)).toBeInTheDocument();
  });
});
