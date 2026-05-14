// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../ipc/invoke', () => ({ invoke: vi.fn() }));
import { invoke } from '../ipc/invoke';
const mockInvoke = vi.mocked(invoke);

import ModalProjectContext, { buildVoiceGuide } from './ModalProjectContext';

const defaultProps = {
  workspaceId: 'ws-abc',
  workspaceName: 'My Project',
  onNext: vi.fn(),
  onBack: vi.fn(),
};

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockResolvedValue(undefined);
});

// ── rendering ────────────────────────────────────────────────────────────────

describe('ModalProjectContext — rendering', () => {
  it('test_renders_identity_field', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByLabelText(/identity/i)).toBeDefined();
  });

  it('test_renders_audience_field', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByLabelText(/audience/i)).toBeDefined();
  });

  it('test_renders_tone_field', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByLabelText(/tone/i)).toBeDefined();
  });

  it('test_renders_avoid_field', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByLabelText(/avoid/i)).toBeDefined();
  });

  it('test_renders_examples_field', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByLabelText(/example posts/i)).toBeDefined();
  });

  it('test_renders_next_button', () => {
    render(<ModalProjectContext {...defaultProps} />);
    expect(screen.getByRole('button', { name: /next/i })).toBeDefined();
  });
});

// ── save on empty: professional defaults ─────────────────────────────────────

describe('ModalProjectContext — empty fields use professional defaults', () => {
  it('test_clicking_next_with_empty_fields_calls_save_project_voice_guide', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_project_voice_guide', expect.objectContaining({
        projectId: 'ws-abc',
      }));
    });
  });

  it('test_empty_fields_produce_professional_tone_in_voice_guide', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('Direct and professional');
    });
  });

  it('test_empty_fields_produce_developer_audience_in_voice_guide', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('developers');
    });
  });

  it('test_clicking_next_with_empty_fields_calls_onNext', async () => {
    const onNext = vi.fn();
    render(<ModalProjectContext {...defaultProps} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
  });
});

// ── save with user input ──────────────────────────────────────────────────────

describe('ModalProjectContext — voice guide field content', () => {
  it('test_description_appears_in_voice_guide_identity_section', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/identity/i), 'Building dev tools in public');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('Building dev tools in public');
    });
  });

  it('test_custom_audience_replaces_default_in_voice_guide', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/audience/i), 'Open-source maintainers');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('Open-source maintainers');
    });
  });

  it('test_custom_tone_replaces_default_in_voice_guide', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/tone/i), 'Dry and technical');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('Dry and technical');
    });
  });
});

describe('ModalProjectContext — avoid and examples fields', () => {
  it('test_avoid_phrases_appear_after_standard_seven', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/avoid/i), 'em dashes');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('em dashes');
    });
  });

  it('test_examples_appear_in_voice_guide_examples_section', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/example posts/i), 'Shipped feature X today.');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('Shipped feature X today.');
    });
  });

  it('test_voice_guide_always_includes_the_standard_seven_phrases', async () => {
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      const guide = (call?.[1] as Record<string, string>)?.voiceGuide ?? '';
      expect(guide).toContain('excited to share');
      expect(guide).toContain('game-changing');
      expect(guide).toContain('seamlessly');
    });
  });
});

// ── load on mount ─────────────────────────────────────────────────────────────

describe('ModalProjectContext — load on mount', () => {
  it('test_fetches_voice_guide_fields_on_mount', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<ModalProjectContext {...defaultProps} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_voice_guide_fields', { projectId: 'ws-abc' });
    });
  });

  it('test_pre_populates_form_when_fields_exist', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_voice_guide_fields') {
        return Promise.resolve({ description: 'My startup', tone: 'Casual', audience: 'Founders', avoid: '', examples: '' });
      }
      return Promise.resolve(undefined);
    });
    render(<ModalProjectContext {...defaultProps} />);
    await waitFor(() => {
      expect((screen.getByLabelText(/identity/i) as HTMLInputElement).value).toBe('My startup');
    });
    expect((screen.getByLabelText(/tone/i) as HTMLTextAreaElement).value).toBe('Casual');
  });

  it('test_does_not_fail_when_fields_fetch_returns_null', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_voice_guide_fields') return Promise.resolve(null);
      return Promise.resolve(undefined);
    });
    render(<ModalProjectContext {...defaultProps} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_voice_guide_fields', expect.anything());
    });
    expect(screen.getByLabelText(/identity/i)).toBeDefined();
  });

  it('test_does_not_fail_when_fields_fetch_throws', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_voice_guide_fields') return Promise.reject(new Error('network'));
      return Promise.resolve(undefined);
    });
    render(<ModalProjectContext {...defaultProps} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_voice_guide_fields', expect.anything());
    });
    expect(screen.getByLabelText(/identity/i)).toBeDefined();
  });
});

// ── save includes fields ───────────────────────────────────────────────────────

describe('ModalProjectContext — save includes voice_guide_fields', () => {
  it('test_save_sends_voice_guide_fields_with_save', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.type(screen.getByLabelText(/identity/i), 'Postlane');
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      const call = mockInvoke.mock.calls.find(c => c[0] === 'save_project_voice_guide');
      expect(call).toBeDefined();
      const args = call?.[1] as Record<string, unknown>;
      expect(args?.voiceGuideFields).toBeDefined();
      expect((args?.voiceGuideFields as Record<string, string>)?.description).toBe('Postlane');
    });
  });
});

// ── error handling ────────────────────────────────────────────────────────────

describe('ModalProjectContext — save error handling', () => {
  it('test_shows_error_message_when_save_fails', async () => {
    mockInvoke.mockRejectedValue(new Error('network error'));
    render(<ModalProjectContext {...defaultProps} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => {
      expect(screen.getByText(/could not save/i)).toBeDefined();
    });
  });

  it('test_still_calls_onNext_after_save_failure', async () => {
    mockInvoke.mockRejectedValue(new Error('network error'));
    const onNext = vi.fn();
    render(<ModalProjectContext {...defaultProps} onNext={onNext} />);
    await userEvent.click(screen.getByRole('button', { name: /next/i }));
    await waitFor(() => expect(onNext).toHaveBeenCalledOnce());
  });
});

// ── buildVoiceGuide unit tests ────────────────────────────────────────────────

describe('buildVoiceGuide — unit', () => {
  it('test_empty_inputs_produce_professional_defaults', () => {
    const guide = buildVoiceGuide({ description: '', audience: '', tone: '', avoid: '', examples: '' }, 'Workspace');
    expect(guide).toContain('Direct and professional');
    expect(guide).toContain('developers');
  });

  it('test_workspace_name_appears_in_header', () => {
    const guide = buildVoiceGuide({ description: '', audience: '', tone: '', avoid: '', examples: '' }, 'Postlane');
    expect(guide).toContain('Postlane');
  });

  it('test_description_goes_in_identity_section', () => {
    const guide = buildVoiceGuide({ description: 'My project', audience: '', tone: '', avoid: '', examples: '' }, 'W');
    expect(guide).toContain('## Identity');
    expect(guide).toContain('My project');
  });

  it('test_no_identity_section_when_description_empty', () => {
    const guide = buildVoiceGuide({ description: '', audience: '', tone: '', avoid: '', examples: '' }, 'W');
    expect(guide).not.toContain('## Identity');
  });

  it('test_no_examples_section_when_examples_empty', () => {
    const guide = buildVoiceGuide({ description: '', audience: '', tone: '', avoid: '', examples: '' }, 'W');
    expect(guide).not.toContain('## Example posts');
  });

  it('test_examples_section_present_when_examples_provided', () => {
    const guide = buildVoiceGuide({ description: '', audience: '', tone: '', avoid: 'em dashes\ncorporate speak', examples: '' }, 'W');
    expect(guide).toContain('em dashes');
    expect(guide).toContain('corporate speak');
  });
});
