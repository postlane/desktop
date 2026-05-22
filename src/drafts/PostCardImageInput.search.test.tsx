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
describe('PostCardImageInput search mode — search and modal', () => {
  it('calls search_unsplash when Search button is clicked', async () => {
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('search_unsplash', { query: 'nature', page: 1 });
    });
  });

  it('opens the picker modal when results are returned', async () => {
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByAltText(/a test photo/i)).toBeInTheDocument();
    expect(document.querySelector('.modal.is-active')).toBeInTheDocument();
  });

  it('does not show photographer names in the modal', async () => {
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    await screen.findByAltText(/a test photo/i);
    expect(screen.queryByText(/jane doe/i)).not.toBeInTheDocument();
  });

  it('calls onSelectUnsplash and closes the modal when a thumbnail is clicked', async () => {
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
    expect(document.querySelector('.modal.is-active')).not.toBeInTheDocument();
  });

  it('closes the modal without selecting when the background is clicked', async () => {
    mockInvoke.mockResolvedValue([PHOTO]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    await screen.findByAltText(/a test photo/i);
    const bg = document.querySelector('.modal-background');
    if (!bg) throw new Error('.modal-background not found');
    fireEvent.click(bg);
    expect(document.querySelector('.modal.is-active')).not.toBeInTheDocument();
  });
});

describe('PostCardImageInput search mode — pagination and errors', () => {
  it('fetches the next page and appends results when Load more is clicked', async () => {
    const PHOTO2 = { ...PHOTO, id: 'xyz', description: 'Second photo' };
    mockInvoke
      .mockResolvedValueOnce([PHOTO])
      .mockResolvedValueOnce([PHOTO2]);
    render(<PostCardImageInput {...defaultProps} />);
    const input = await screen.findByRole('searchbox', { name: /search unsplash/i });
    fireEvent.change(input, { target: { value: 'nature' } });
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    await screen.findByAltText(/a test photo/i);
    fireEvent.click(screen.getByRole('button', { name: /load more/i }));
    expect(await screen.findByAltText(/second photo/i)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenLastCalledWith('search_unsplash', { query: 'nature', page: 2 });
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

  it('Search button is enabled when the query is empty', () => {
    render(<PostCardImageInput {...defaultProps} />);
    expect(screen.getByRole('button', { name: /search images/i })).not.toBeDisabled();
  });

  it('shows a red error when Search is clicked with an empty query', async () => {
    render(<PostCardImageInput {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByRole('alert')).toHaveTextContent(/enter a search term/i);
  });

  it('clears the empty-query error when the user types in the search box', async () => {
    render(<PostCardImageInput {...defaultProps} />);
    fireEvent.click(screen.getByRole('button', { name: /search images/i }));
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    fireEvent.change(screen.getByRole('searchbox', { name: /search unsplash/i }), { target: { value: 'sunset' } });
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
