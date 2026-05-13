// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@testing-library/jest-dom';
import SubstackNotesCard from './SubstackNotesCard';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));

// ---------------------------------------------------------------------------
// SubstackNotesCard — render
// ---------------------------------------------------------------------------

describe('SubstackNotesCard — render', () => {
  it('renders_content_in_read_mode', () => {
    render(<SubstackNotesCard content="Hello Substack" />);
    expect(screen.getByText('Hello Substack')).toBeInTheDocument();
  });

  it('renders_with_no_props_without_crashing', () => {
    render(<SubstackNotesCard />);
    expect(screen.getByText('0/300')).toBeInTheDocument();
  });

  it('shows_character_counter_with_correct_limit', () => {
    render(<SubstackNotesCard content="Hello" />);
    expect(screen.getByText('5/300')).toBeInTheDocument();
  });

  it('shows_zero_counter_when_content_is_empty_string', () => {
    render(<SubstackNotesCard content="" />);
    expect(screen.getByText('0/300')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// SubstackNotesCard — image rendering
// ---------------------------------------------------------------------------

describe('SubstackNotesCard — image rendering', () => {
  it('renders_image_when_imageUrl_is_provided', () => {
    const { container } = render(
      <SubstackNotesCard content="Post text" imageUrl="https://example.com/og.png" />,
    );
    const img = container.querySelector('img');
    expect(img).toBeInTheDocument();
    expect(img).toHaveAttribute('src', 'https://example.com/og.png');
  });

  it('does_not_render_img_when_imageUrl_is_absent', () => {
    const { container } = render(<SubstackNotesCard content="Post text" />);
    expect(container.querySelector('img')).not.toBeInTheDocument();
  });

  it('image_has_correct_alt_text', () => {
    render(<SubstackNotesCard imageUrl="https://example.com/img.png" />);
    expect(screen.getByRole('img')).toHaveAttribute('alt', 'Post image');
  });

  it('hides_image_while_editing', () => {
    const { container } = render(
      <SubstackNotesCard
        content="Post"
        imageUrl="https://example.com/img.png"
        onSave={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(container.querySelector('img')).not.toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// SubstackNotesCard — character counter states
// ---------------------------------------------------------------------------

describe('SubstackNotesCard — character counter states', () => {
  it('counter_is_grey_when_under_limit', () => {
    render(<SubstackNotesCard content={'a'.repeat(299)} />);
    const counter = screen.getByText('299/300');
    expect(counter).toHaveClass('has-text-grey');
    expect(counter).not.toHaveClass('has-text-danger');
  });

  it('counter_is_grey_at_exactly_300_chars', () => {
    render(<SubstackNotesCard content={'a'.repeat(300)} />);
    const counter = screen.getByText('300/300');
    expect(counter).toHaveClass('has-text-grey');
    expect(counter).not.toHaveClass('has-text-danger');
  });

  it('counter_is_red_when_over_limit', () => {
    render(<SubstackNotesCard content={'a'.repeat(301)} />);
    const counter = screen.getByText('301/300');
    expect(counter).toHaveClass('has-text-danger');
    expect(counter).not.toHaveClass('has-text-grey');
  });

  it('counter_uses_is_size_7_in_both_states', () => {
    const { rerender } = render(<SubstackNotesCard content={'a'.repeat(10)} />);
    expect(screen.getByText('10/300')).toHaveClass('is-size-7');

    rerender(<SubstackNotesCard content={'a'.repeat(301)} />);
    expect(screen.getByText('301/300')).toHaveClass('is-size-7');
  });
});

// ---------------------------------------------------------------------------
// SubstackNotesCard — inline edit (covers lines 35-36, 47-60)
// ---------------------------------------------------------------------------

describe('SubstackNotesCard — inline edit', () => {
  it('shows_edit_button_when_onSave_is_provided', () => {
    render(<SubstackNotesCard content="Original" onSave={vi.fn()} />);
    expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument();
  });

  it('does_not_show_edit_button_when_onSave_is_absent', () => {
    render(<SubstackNotesCard content="Original" />);
    expect(screen.queryByRole('button', { name: /edit/i })).not.toBeInTheDocument();
  });

  it('clicking_edit_reveals_textarea_prefilled_with_content', () => {
    render(<SubstackNotesCard content="Original post" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(screen.getByRole('textbox')).toHaveValue('Original post');
  });

  it('character_counter_updates_live_as_user_types', () => {
    render(<SubstackNotesCard content="Hello" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Hi' } });
    expect(screen.getByText('2/300')).toBeInTheDocument();
  });

  it('save_calls_onSave_with_edited_content', () => {
    const onSave = vi.fn();
    render(<SubstackNotesCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(onSave).toHaveBeenCalledWith('Updated');
  });

  it('save_returns_to_read_mode_and_hides_textarea', () => {
    const onSave = vi.fn();
    render(<SubstackNotesCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Updated' } });
    fireEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /edit/i })).toBeInTheDocument();
  });
});

describe('SubstackNotesCard — inline edit cancel and validation', () => {
  it('cancel_returns_to_read_mode_without_calling_onSave', () => {
    const onSave = vi.fn();
    render(<SubstackNotesCard content="Original" onSave={onSave} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Changed' } });
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('cancel_reverts_displayed_content_to_original', () => {
    render(<SubstackNotesCard content="Original" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Changed' } });
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(screen.getByText('Original')).toBeInTheDocument();
  });

  it('textarea_has_rows_1_and_is_small_class', () => {
    render(<SubstackNotesCard content="Post" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    const textarea = screen.getByRole('textbox');
    expect(textarea).toHaveAttribute('rows', '1');
    expect(textarea).toHaveClass('textarea', 'is-small');
  });

  it('save_is_disabled_when_draft_exceeds_300_chars', () => {
    render(<SubstackNotesCard content="Original" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(301) } });
    expect(screen.getByRole('button', { name: /save/i })).toBeDisabled();
  });

  it('save_is_enabled_at_exactly_300_chars', () => {
    render(<SubstackNotesCard content="Original" onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a'.repeat(300) } });
    expect(screen.getByRole('button', { name: /save/i })).not.toBeDisabled();
  });

  it('counter_shows_draft_count_during_editing_not_content_count', () => {
    render(<SubstackNotesCard content={'a'.repeat(100)} onSave={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'Hi' } });
    expect(screen.getByText('2/300')).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// SubstackNotesCard — action buttons
// ---------------------------------------------------------------------------

describe('SubstackNotesCard — action buttons', () => {
  it('calls_onApprove_when_approve_button_clicked', () => {
    const onApprove = vi.fn();
    render(<SubstackNotesCard content="Post" onApprove={onApprove} />);
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    expect(onApprove).toHaveBeenCalledOnce();
  });

  it('calls_onDelete_when_delete_button_clicked', () => {
    const onDelete = vi.fn();
    render(<SubstackNotesCard content="Post" onDelete={onDelete} />);
    fireEvent.click(screen.getByRole('button', { name: /delete/i }));
    expect(onDelete).toHaveBeenCalledOnce();
  });

  it('calls_onImageClick_when_image_button_clicked', () => {
    const onImageClick = vi.fn();
    render(<SubstackNotesCard content="Post" onImageClick={onImageClick} />);
    fireEvent.click(screen.getByRole('button', { name: /image/i }));
    expect(onImageClick).toHaveBeenCalledOnce();
  });

  it('shows_custom_approveLabel', () => {
    render(<SubstackNotesCard content="Post" onApprove={vi.fn()} approveLabel="Publish" />);
    expect(screen.getByRole('button', { name: /publish/i })).toBeInTheDocument();
  });

  it('approve_uses_default_label_when_approveLabel_not_provided', () => {
    render(<SubstackNotesCard content="Post" onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).toBeInTheDocument();
  });

  it('approve_is_disabled_when_content_exceeds_300_chars', () => {
    render(<SubstackNotesCard content={'a'.repeat(301)} onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).toBeDisabled();
  });

  it('approve_is_enabled_at_exactly_300_chars', () => {
    render(<SubstackNotesCard content={'a'.repeat(300)} onApprove={vi.fn()} />);
    expect(screen.getByRole('button', { name: /approve/i })).not.toBeDisabled();
  });
});
