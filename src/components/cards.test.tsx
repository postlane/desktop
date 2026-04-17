// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import XCard from './XCard';
import BlueskyCard from './BlueskyCard';
import MastodonCard from './MastodonCard';
import PostPreview from './PostPreview';

// 50-character URL: https:// (8) + example.com/ (12) + 30 a's = 50 chars.
// Matches parser.rs regex: https?://[^\s]+
const URL_50 = `https://example.com/${'a'.repeat(30)}`;

// ---------------------------------------------------------------------------
// 5.10.1 — XCard URL counting
// ---------------------------------------------------------------------------

describe('XCard', () => {
  it('counts a 50-char URL as 23 characters (t.co wrapping rule)', () => {
    render(<XCard content={URL_50} />);
    expect(screen.getByText('23/280')).toBeInTheDocument();
  });

  // 5.10.5
  it('shows red counter when content exceeds 280 chars', () => {
    render(<XCard content={'a'.repeat(281)} />);
    expect(screen.getByText('281/280')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// 5.10.2 — BlueskyCard URL counting
// ---------------------------------------------------------------------------

describe('BlueskyCard', () => {
  it('counts a 50-char URL as 50 characters (full URL length)', () => {
    render(<BlueskyCard content={URL_50} />);
    expect(screen.getByText('50/300')).toBeInTheDocument();
  });

  it('does not italicise underscores inside URLs (UTM params)', () => {
    const { container } = render(
      <BlueskyCard content="https://postlane.dev?utm_source=bluesky&utm_medium=social&utm_content=20260417" />
    );
    expect(container.querySelector('em')).not.toBeInTheDocument();
  });

  // 5.10.6
  it('shows red counter when content exceeds 300 chars', () => {
    render(<BlueskyCard content={'a'.repeat(301)} />);
    expect(screen.getByText('301/300')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// 5.10.3 — MastodonCard URL counting
// ---------------------------------------------------------------------------

describe('MastodonCard', () => {
  it('counts a 50-char URL as 23 characters (Mastodon URL counting rule)', () => {
    render(<MastodonCard content={URL_50} />);
    expect(screen.getByText('23/500')).toBeInTheDocument();
  });

  // 5.10.7
  it('shows red counter when content exceeds 500 chars', () => {
    render(<MastodonCard content={'a'.repeat(501)} />);
    expect(screen.getByText('501/500')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Image rendering inside cards
// ---------------------------------------------------------------------------

describe('XCard — image rendering', () => {
  it('renders the image between text and counter when imageUrl is provided', () => {
    const { container } = render(
      <XCard content="Post text" imageUrl="https://example.com/og.png" />
    );
    const img = container.querySelector('img');
    expect(img).toBeInTheDocument();
    expect(img).toHaveAttribute('src', 'https://example.com/og.png');
  });

  it('does not render an img element when imageUrl is absent', () => {
    const { container } = render(<XCard content="Post text" />);
    expect(container.querySelector('img')).not.toBeInTheDocument();
  });
});

describe('BlueskyCard — image rendering', () => {
  it('renders the image when imageUrl is provided', () => {
    const { container } = render(
      <BlueskyCard content="Post text" imageUrl="https://example.com/og.png" />
    );
    expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.com/og.png');
  });
});

describe('MastodonCard — image rendering', () => {
  it('renders the image when imageUrl is provided', () => {
    const { container } = render(
      <MastodonCard content="Post text" imageUrl="https://example.com/og.png" />
    );
    expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.com/og.png');
  });
});

// ---------------------------------------------------------------------------
// Inline edit — XCard
// ---------------------------------------------------------------------------

describe('XCard — inline edit', () => {
  it('shows Edit button when onSave is provided', () => {
    render(<XCard content="Original" onSave={vi.fn()} />);
    expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument();
  });

  it('does not show Edit button when onSave is not provided', () => {
    render(<XCard content="Original" />);
    expect(screen.queryByRole('button', { name: /edit/i })).not.toBeInTheDocument();
  });

  it('clicking Edit reveals a textarea pre-filled with current content', () => {
    render(<XCard content="Original post" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(screen.getByRole('textbox')).toHaveValue('Original post');
  });

  it('character counter updates live as user types in the textarea', () => {
    render(<XCard content="Hello" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Hi' } });
    expect(screen.getByText('2/280')).toBeInTheDocument();
  });

  it('Save calls onSave with the edited content', () => {
    const onSave = vi.fn();
    render(<XCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith('Updated');
  });

  it('Cancel reverts to read-only mode without calling onSave', () => {
    const onSave = vi.fn();
    render(<XCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Changed' } });
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Inline edit — BlueskyCard and MastodonCard (abbreviated — same contract)
// ---------------------------------------------------------------------------

describe('BlueskyCard — inline edit', () => {
  it('shows Edit button when onSave is provided', () => {
    render(<BlueskyCard content="Original" onSave={vi.fn()} />);
    expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument();
  });

  it('Save calls onSave with edited content', () => {
    const onSave = vi.fn();
    render(<BlueskyCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith('Updated');
  });
});

describe('MastodonCard — inline edit', () => {
  it('shows Edit button when onSave is provided', () => {
    render(<MastodonCard content="Original" onSave={vi.fn()} />);
    expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument();
  });

  it('Save calls onSave with edited content', () => {
    const onSave = vi.fn();
    render(<MastodonCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith('Updated');
  });
});

// ---------------------------------------------------------------------------
// 5.10.4 — PostPreview smoke test
// ---------------------------------------------------------------------------

describe('PostPreview', () => {
  it('renders without crashing for all three platforms with default props', () => {
    const { unmount: u1 } = render(<PostPreview />);
    u1();
    const { unmount: u2 } = render(<PostPreview platform="x" content="Hello world" />);
    u2();
    const { unmount: u3 } = render(<PostPreview platform="bluesky" content="Hello world" />);
    u3();
    render(<PostPreview platform="mastodon" content="Hello world" />);
  });
});
