// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import XCard from './XCard';
import BlueskyCard from './BlueskyCard';
import MastodonCard from './MastodonCard';
import LinkedInCard from './LinkedInCard';
import PostPreview from './PostPreview';
import { countCharsX, countCharsBluesky, countCharsMastodon } from './charCount';

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

  it('renders **bold** as <strong>', () => {
    const { container } = render(<BlueskyCard content="**bold text**" />);
    expect(container.querySelector('strong')).toHaveTextContent('bold text');
  });

  it('renders _italic_ as <em>', () => {
    const { container } = render(<BlueskyCard content="_italic text_" />);
    expect(container.querySelector('em')).toHaveTextContent('italic text');
  });

  it('Cancel returns to read mode without saving', () => {
    const onSave = vi.fn();
    render(<BlueskyCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
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

// ---------------------------------------------------------------------------
// MastodonCard — content warning
// ---------------------------------------------------------------------------

describe('MastodonCard — content warning', () => {
  it('renders CW bar when content starts with "CW:"', () => {
    render(<MastodonCard content={'CW: spoilers\nThe actual body'} />);
    expect(screen.getByText(/CW: spoilers/)).toBeInTheDocument();
  });

  it('does not render CW bar when content has no CW prefix', () => {
    render(<MastodonCard content="Normal post" />);
    expect(screen.queryByText(/^CW:/)).not.toBeInTheDocument();
  });

  it('hides CW bar while editing', () => {
    const { container } = render(<MastodonCard content={'CW: spoilers\nBody'} onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(container.querySelector('.bg-amber-50')).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// MastodonCard — HTML rendering (parseMastodonHTML)
// ---------------------------------------------------------------------------

describe('MastodonCard — HTML rendering', () => {
  it('renders <b> as <strong>', () => {
    const { container } = render(<MastodonCard content="<b>bold text</b>" />);
    expect(container.querySelector('strong')).toHaveTextContent('bold text');
  });

  it('renders <i> as <em>', () => {
    const { container } = render(<MastodonCard content="<i>italic text</i>" />);
    expect(container.querySelector('em')).toHaveTextContent('italic text');
  });

  it('renders <a href="https://..."> as a link', () => {
    const { container } = render(
      <MastodonCard content='<a href="https://postlane.dev">Postlane</a>' />,
    );
    const link = container.querySelector('a');
    expect(link).toHaveAttribute('href', 'https://postlane.dev');
    expect(link).toHaveTextContent('Postlane');
  });

  it('does not render <a> when href is not https', () => {
    const { container } = render(
      <MastodonCard content='<a href="http://example.com">insecure</a>' />,
    );
    expect(container.querySelector('a')).not.toBeInTheDocument();
  });

  it('preserves plain text before and after tags (multiple nodes)', () => {
    const { container } = render(
      <MastodonCard content="Before <b>bold</b> after" />,
    );
    const div = container.querySelector('.whitespace-pre-wrap');
    expect(div?.textContent).toContain('Before');
    expect(div?.textContent).toContain('bold');
    expect(div?.textContent).toContain('after');
  });

  it('renders plain text with no tags as a single text node', () => {
    render(<MastodonCard content="Just plain text" />);
    expect(screen.getByText('Just plain text')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// MastodonCard — action buttons
// ---------------------------------------------------------------------------

describe('MastodonCard — action buttons', () => {
  it('calls onApprove when Approve clicked', () => {
    const onApprove = vi.fn();
    render(<MastodonCard content="Post" onApprove={onApprove} />);
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    expect(onApprove).toHaveBeenCalledOnce();
  });

  it('calls onDelete when Delete clicked', () => {
    const onDelete = vi.fn();
    render(<MastodonCard content="Post" onDelete={onDelete} />);
    fireEvent.click(screen.getByRole('button', { name: /delete/i }));
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it('calls onImageClick when Image clicked', () => {
    const onImageClick = vi.fn();
    render(<MastodonCard content="Post" onImageClick={onImageClick} />);
    fireEvent.click(screen.getByRole('button', { name: /image/i }));
    expect(onImageClick).toHaveBeenCalledOnce();
  });

  it('shows custom approveLabel', () => {
    render(<MastodonCard content="Post" onApprove={vi.fn()} approveLabel="Retry" />);
    expect(screen.getByRole('button', { name: /retry/i })).toBeInTheDocument();
  });

  it('Approve is disabled when content exceeds 500 chars', () => {
    render(<MastodonCard content={'a'.repeat(501)} onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).toBeDisabled();
  });

  it('renders image when imageUrl provided', () => {
    const { container } = render(
      <MastodonCard content="Post" imageUrl="https://example.com/img.png" />,
    );
    expect(container.querySelector('img')).toHaveAttribute('src', 'https://example.com/img.png');
  });

  it('Cancel button returns to read mode without calling onSave', () => {
    const onSave = vi.fn();
    render(<MastodonCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});

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

  it('renders LinkedInCard when platform is linkedin', () => {
    render(<PostPreview platform="linkedin" content="Professional post" />);
    expect(screen.getByText('Professional post')).toBeInTheDocument();
    // LinkedIn limit is 3000 — counter must show /3000 not /280
    expect(screen.getByText('17/3000')).toBeInTheDocument();
  });

  it('renders SubstackNotesCard when platform is substack_notes', () => {
    render(<PostPreview platform="substack_notes" content="Hello" />);
    // Substack Notes limit is 300 — must not fall through to XCard showing /280
    expect(screen.getByText('5/300')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// 8.2 / 8.3 — LinkedInCard
// ---------------------------------------------------------------------------

describe('LinkedInCard — URL counting', () => {
  it('counts a 50-char URL at full length (no t.co collapsing)', () => {
    render(<LinkedInCard content={URL_50} />);
    expect(screen.getByText('50/3000')).toBeInTheDocument();
  });

  it('counts 3000 chars as exactly at limit', () => {
    render(<LinkedInCard content={'a'.repeat(3000)} />);
    expect(screen.getByText('3000/3000')).toBeInTheDocument();
  });

  it('shows red counter at 3001 chars', () => {
    render(<LinkedInCard content={'a'.repeat(3001)} />);
    expect(screen.getByText('3001/3000')).toBeInTheDocument();
  });
});

describe('LinkedInCard — approve button', () => {
  it('Approve is disabled when content exceeds 3000 chars', () => {
    render(<LinkedInCard content={'a'.repeat(3001)} onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).toBeDisabled();
  });

  it('Approve is enabled at exactly 3000 chars', () => {
    render(<LinkedInCard content={'a'.repeat(3000)} onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).not.toBeDisabled();
  });
});

describe('LinkedInCard — avatar shape', () => {
  it('renders without crashing with default props', () => {
    render(<LinkedInCard />);
    expect(screen.getByText('0/3000')).toBeInTheDocument();
  });

  it('avatar uses rounded-md (square with rounded corners, not a circle)', () => {
    const { container } = render(<LinkedInCard />);
    const avatar = container.querySelector('.rounded-md');
    expect(avatar).toBeInTheDocument();
    expect(container.querySelector('.rounded-full')).not.toBeInTheDocument();
  });
});

describe('LinkedInCard — inline edit', () => {
  it('clicking Edit reveals a textarea pre-filled with current content', () => {
    render(<LinkedInCard content="My LinkedIn post" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(screen.getByRole('textbox')).toHaveValue('My LinkedIn post');
  });

  it('character counter updates as user types', () => {
    render(<LinkedInCard content="Hello" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Hi' } });
    expect(screen.getByText('2/3000')).toBeInTheDocument();
  });

  it('Save calls onSave with edited content', () => {
    const onSave = vi.fn();
    render(<LinkedInCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith('Updated');
  });

  it('Cancel returns to read mode without calling onSave', () => {
    const onSave = vi.fn();
    render(<LinkedInCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });
});

// 8.3.1 — LinkedInCard author display
describe('LinkedInCard — author display', () => {
  it('renders author name when provided', () => {
    render(<LinkedInCard authorName="Jane Doe" authorHandle="janedoe" />);
    expect(screen.getByText('Jane Doe')).toBeInTheDocument();
  });

  it('renders author handle with · 1st suffix', () => {
    render(<LinkedInCard authorName="Jane Doe" authorHandle="janedoe" />);
    expect(screen.getByText('janedoe · 1st')).toBeInTheDocument();
  });
});

// Critical fix: Unicode counting — all platforms must count emoji as 1 char
// Low: emoji near 3000-char limit for LinkedIn
describe('countLinkedInChars — emoji at limit', () => {
  it('counts emoji as 1 character so 2999 ascii + emoji = 3000, not 3001', () => {
    const content = 'a'.repeat(2999) + '🎉';
    render(<LinkedInCard content={content} />);
    expect(screen.getByText('3000/3000')).toBeInTheDocument();
  });
});

describe('countCharsX — Unicode', () => {
  it('counts emoji as 1 character, not 2 UTF-16 code units', () => {
    expect(countCharsX('Hello 🎉')).toBe(7);
  });
  it('counts emoji correctly alongside URL replacement', () => {
    // emoji(1) + space(1) + URL(50 → 23) = 25
    expect(countCharsX(`🎉 ${URL_50}`)).toBe(25);
  });
});

describe('countCharsBluesky — Unicode', () => {
  it('counts emoji as 1 character, not 2 UTF-16 code units', () => {
    expect(countCharsBluesky('Hello 🎉')).toBe(7);
  });
});

describe('countCharsMastodon — Unicode', () => {
  it('counts emoji as 1 character, not 2 UTF-16 code units', () => {
    expect(countCharsMastodon('Hello 🎉')).toBe(7);
  });
});

// ---------------------------------------------------------------------------
// 9.2.3 — MastodonCard dynamic charLimit prop
// ---------------------------------------------------------------------------

describe('MastodonCard — dynamic charLimit', () => {
  it('uses charLimit prop instead of hardcoded 500', () => {
    render(<MastodonCard content="Hello" charLimit={2000} />);
    expect(screen.getByText('5/2000')).toBeInTheDocument();
  });

  it('turns red counter when content exceeds charLimit', () => {
    render(<MastodonCard content={'a'.repeat(2001)} charLimit={2000} />);
    expect(screen.getByText('2001/2000')).toBeInTheDocument();
  });

  it('defaults to 500 when charLimit is not provided', () => {
    render(<MastodonCard content="Hi" />);
    expect(screen.getByText('2/500')).toBeInTheDocument();
  });

  it('amber threshold is charLimit minus 50', () => {
    render(<MastodonCard content={'a'.repeat(1951)} charLimit={2000} />);
    expect(screen.getByText('1951/2000')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Issue 8 — PostPreview must thread charLimit to MastodonCard
// ---------------------------------------------------------------------------

describe('PostPreview — mastodon charLimit threading', () => {
  it('passes charLimit to MastodonCard so instance-specific limits are shown', () => {
    render(<PostPreview platform="mastodon" content="Hi" charLimit={300} />);
    expect(screen.getByText('2/300')).toBeInTheDocument();
  });

  it('uses MastodonCard default (500) when charLimit is omitted', () => {
    render(<PostPreview platform="mastodon" content="Hi" />);
    expect(screen.getByText('2/500')).toBeInTheDocument();
  });
});
