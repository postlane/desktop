// SPDX-License-Identifier: BUSL-1.1
// Tests for §22.7 AccountDangerZone component.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import AccountDangerZone from './AccountDangerZone';

const mockInvoke = vi.fn();
vi.mock('../ipc/invoke', () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));
vi.mock('./AccountDeletionProgress', () => ({
  default: ({ deleteWorkspaceDirs }: { deleteWorkspaceDirs: boolean }) => (
    <div data-testid="deletion-progress" data-delete-dirs={String(deleteWorkspaceDirs)} />
  ),
}));

const USER_EMAIL = 'hugo@example.com';

beforeEach(() => {
  vi.clearAllMocks();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_deletion_incomplete') return Promise.resolve(false);
    return Promise.resolve(null);
  });
});

// ── 22.7.1: Danger Zone section ───────────────────────────────────────────────

describe('22.7.1: Danger Zone section', () => {
  it('renders a Danger Zone section for the account', async () => {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await waitFor(() => expect(screen.queryByText(/Danger Zone/i)).not.toBeNull());
  });

  it('is collapsed by default', async () => {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    expect(screen.queryByText(/Delete my Postlane account/i)).toBeNull();
  });

  it('shows the delete button after expanding', async () => {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => expect(screen.queryByText(/Delete my Postlane account/i)).not.toBeNull());
  });
});

// ── 22.7.2: email confirmation input ─────────────────────────────────────────

describe('22.7.2: email confirmation input', () => {
  async function expandAndOpen() {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => screen.getByRole('button', { name: /Delete my Postlane account/i }));
    fireEvent.click(screen.getByRole('button', { name: /Delete my Postlane account/i }));
    await act(async () => {});
  }

  it('shows email confirmation input', async () => {
    await expandAndOpen();
    await waitFor(() => expect(screen.getByPlaceholderText(/Type your account email to confirm/i)).toBeDefined());
  });

  it('shows the account identifier in the confirmation label', async () => {
    await expandAndOpen();
    await waitFor(() => expect(screen.getByText(USER_EMAIL)).not.toBeNull());
  });

  it('wraps the identifier in quotes so the colon is not mistakenly included', async () => {
    await expandAndOpen();
    await waitFor(() =>
      expect(screen.queryByText((_, el) =>
        el?.tagName === 'LABEL' && (el?.textContent ?? '').includes(`"${USER_EMAIL}"`)
      )).not.toBeNull()
    );
  });

  it('Delete button disabled when input is empty', async () => {
    await expandAndOpen();
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
    const btn = screen.getByRole('button', { name: /^Delete my account$/i });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
  });

  it('Delete button disabled when email does not match', async () => {
    await expandAndOpen();
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
    fireEvent.change(screen.getByPlaceholderText(/Type your account email/i), {
      target: { value: 'wrong@example.com' },
    });
    expect((screen.getByRole('button', { name: /^Delete my account$/i }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('Delete button enabled when email matches exactly (case-insensitive)', async () => {
    await expandAndOpen();
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
    fireEvent.change(screen.getByPlaceholderText(/Type your account email/i), {
      target: { value: USER_EMAIL.toUpperCase() },
    });
    expect((screen.getByRole('button', { name: /^Delete my account$/i }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('Delete button re-disabled when email is changed away', async () => {
    await expandAndOpen();
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
    const input = screen.getByPlaceholderText(/Type your account email/i);
    fireEvent.change(input, { target: { value: USER_EMAIL } });
    fireEvent.change(input, { target: { value: USER_EMAIL + 'x' } });
    expect((screen.getByRole('button', { name: /^Delete my account$/i }) as HTMLButtonElement).disabled).toBe(true);
  });
});

// ── 22.7.3: workspace deletion checkbox ──────────────────────────────────────

describe('22.7.3: workspace deletion checkbox', () => {
  async function expandAndOpen() {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => screen.getByRole('button', { name: /Delete my Postlane account/i }));
    fireEvent.click(screen.getByRole('button', { name: /Delete my Postlane account/i }));
    await act(async () => {});
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
  }

  it('checkbox is checked by default', async () => {
    await expandAndOpen();
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
  });

  it('checkbox can be unchecked', async () => {
    await expandAndOpen();
    const checkbox = screen.getByRole('checkbox') as HTMLInputElement;
    fireEvent.click(checkbox);
    expect(checkbox.checked).toBe(false);
  });

  it('shows deletion progress with deleteWorkspaceDirs=true when checkbox checked', async () => {
    await expandAndOpen();
    fireEvent.change(screen.getByPlaceholderText(/Type your account email/i), {
      target: { value: USER_EMAIL },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Delete my account$/i }));
    await waitFor(() => expect(screen.queryByTestId('deletion-progress')).not.toBeNull());
    expect(screen.getByTestId('deletion-progress').getAttribute('data-delete-dirs')).toBe('true');
  });

  it('shows deletion progress with deleteWorkspaceDirs=false when unchecked', async () => {
    await expandAndOpen();
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.change(screen.getByPlaceholderText(/Type your account email/i), {
      target: { value: USER_EMAIL },
    });
    fireEvent.click(screen.getByRole('button', { name: /^Delete my account$/i }));
    await waitFor(() => expect(screen.queryByTestId('deletion-progress')).not.toBeNull());
    expect(screen.getByTestId('deletion-progress').getAttribute('data-delete-dirs')).toBe('false');
  });
});

// ── 22.7.7a: incomplete deletion warning ──────────────────────────────────────

describe('22.7.7a: incomplete deletion warning banner', () => {
  it('shows persistent warning when deletion_incomplete is true', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_deletion_incomplete') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await waitFor(() =>
      expect(screen.queryByText(/previous account deletion was incomplete/i)).not.toBeNull()
    );
  });

  it('does not show warning when deletion_incomplete is false', async () => {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    expect(screen.queryByText(/previous account deletion was incomplete/i)).toBeNull();
  });
});

// ── 22.7.21: email match gates button ─────────────────────────────────────────

describe('22.7.21: Delete button gated on email (case-insensitive)', () => {
  async function expandAndOpen() {
    render(<AccountDangerZone userEmail={USER_EMAIL} onDeleted={vi.fn()} />);
    await act(async () => {});
    fireEvent.click(screen.getByText(/Danger Zone/i));
    await waitFor(() => screen.getByRole('button', { name: /Delete my Postlane account/i }));
    fireEvent.click(screen.getByRole('button', { name: /Delete my Postlane account/i }));
    await act(async () => {});
    await waitFor(() => screen.getByPlaceholderText(/Type your account email/i));
  }

  it('exact lowercase match enables button', async () => {
    await expandAndOpen();
    fireEvent.change(screen.getByPlaceholderText(/Type your account email/i), {
      target: { value: USER_EMAIL },
    });
    expect((screen.getByRole('button', { name: /^Delete my account$/i }) as HTMLButtonElement).disabled).toBe(false);
  });
});
