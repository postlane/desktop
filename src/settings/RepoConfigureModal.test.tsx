// SPDX-License-Identifier: BUSL-1.1

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import RepoConfigureModal from './RepoConfigureModal';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
import { invoke } from '@tauri-apps/api/core';
const mockInvoke = vi.mocked(invoke);

beforeEach(() => vi.clearAllMocks());

describe('RepoConfigureModal — loading state (§15 review fix 6)', () => {
  it('shows a loading indicator before the credential fetch resolves', async () => {
    let resolve: (v: string | null) => void = () => {};
    const pending = new Promise<string | null>((res) => { resolve = res; });
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return pending;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
    resolve(null);
    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument());
  });

  it('does not show "Use default" text while loading', async () => {
    let resolve: (v: string | null) => void = () => {};
    const pending = new Promise<string | null>((res) => { resolve = res; });
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return pending;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    expect(screen.queryByText(/using default credentials/i)).not.toBeInTheDocument();
    resolve(null);
    await waitFor(() => expect(screen.getByText(/using default credentials/i)).toBeInTheDocument());
  });
});

describe('RepoConfigureModal — provider dropdown (§15 review fix 1)', () => {
  it('shows Substack Notes as a provider option in the form', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const select = await screen.findByRole('combobox', { name: /provider/i });
    const options = Array.from(select.querySelectorAll('option')).map((o) => o.textContent);
    expect(options).toContain('Substack Notes');
  });

  it('does not show Webhook as a provider option', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const select = await screen.findByRole('combobox', { name: /provider/i });
    const options = Array.from(select.querySelectorAll('option')).map((o) => o.textContent);
    expect(options).not.toContain('Webhook');
  });
});

describe('RepoConfigureModal — default state', () => {
  it('shows "Use default" when no per-repo credential exists', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/using default credentials/i)).toBeInTheDocument(),
    );
  });

  it('shows a provider selector and key input when "Use a different account" is clicked', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    await waitFor(() =>
      expect(screen.getByRole('combobox', { name: /provider/i })).toBeInTheDocument(),
    );
    expect(screen.getByPlaceholderText(/api key/i)).toBeInTheDocument();
  });
});

describe('RepoConfigureModal — configured state (§15.3.3)', () => {
  it('shows "Using separate account" with masked key when per-repo credential exists', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() =>
      expect(screen.getByText(/using separate account/i)).toBeInTheDocument(),
    );
    expect(screen.getByText(/5678/)).toBeInTheDocument();
  });

  it('shows Remove button when per-repo credential exists', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByRole('button', { name: /remove/i })).toBeInTheDocument());
  });
});

describe('RepoConfigureModal — remove error surface (§15 review fix 8)', () => {
  it('shows an error message when remove_repo_scheduler_key throws', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') throw new Error('Keychain locked');
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(screen.getByText(/keychain locked/i)).toBeInTheDocument(),
    );
  });

  it('does not reset to "Use default" when remove fails', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') throw new Error('Keychain locked');
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => screen.getByText(/keychain locked/i));
    expect(screen.getByText(/using separate account/i)).toBeInTheDocument();
  });
});

describe('RepoConfigureModal — remove flow (§15.3.4)', () => {
  it('clicking Remove calls remove_repo_scheduler_key with repoId and provider', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'remove_repo_scheduler_key',
        expect.objectContaining({ repoId: 'r1', provider: 'zernio' }),
      ),
    );
  });

  it('switches back to "Use default" state after Remove', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() =>
      expect(screen.getByText(/using default credentials/i)).toBeInTheDocument(),
    );
  });
});

describe('RepoConfigureModal — onCredentialChange callback (§15 review fix 4)', () => {
  it('calls onCredentialChange after a successful save', async () => {
    const onCredentialChange = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'save_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} onCredentialChange={onCredentialChange} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const keyInput = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(keyInput, { target: { value: 'sk-test-abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() => expect(onCredentialChange).toHaveBeenCalledOnce());
  });

  it('calls onCredentialChange after a successful remove', async () => {
    const onCredentialChange = vi.fn();
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return '••••••••5678';
      if (cmd === 'remove_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} onCredentialChange={onCredentialChange} />);
    await waitFor(() => screen.getByRole('button', { name: /remove/i }));
    fireEvent.click(screen.getByRole('button', { name: /remove/i }));
    await waitFor(() => expect(onCredentialChange).toHaveBeenCalledOnce());
  });
});

describe('RepoConfigureModal — test connection (§15.2.2 fix 11)', () => {
  it('shows a Test connection button in the form', async () => {
    mockInvoke.mockResolvedValue(null);
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    expect(await screen.findByRole('button', { name: /test connection/i })).toBeInTheDocument();
  });

  it('Test connection button calls test_scheduler with the selected provider', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'test_scheduler') return true;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(await screen.findByRole('button', { name: /test connection/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('test_scheduler', { provider: 'zernio' }),
    );
  });

  it('shows success tick after a passing connection test', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'test_scheduler') return true;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(await screen.findByRole('button', { name: /test connection/i }));
    await waitFor(() => expect(screen.getByText(/✓/)).toBeInTheDocument());
  });
});

describe('RepoConfigureModal — save flow', () => {
  it('Save calls save_repo_scheduler_key with repoId, provider, and key', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'save_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const keyInput = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(keyInput, { target: { value: 'sk-test-abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_repo_scheduler_key',
        expect.objectContaining({ repoId: 'r1', key: 'sk-test-abc123' }),
      ),
    );
  });

  it('shows the masked key after successful Save', async () => {
    mockInvoke.mockImplementation(async (cmd: unknown) => {
      if (cmd === 'get_per_repo_scheduler_key') return null;
      if (cmd === 'save_repo_scheduler_key') return null;
      return null;
    });
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider="zernio" onClose={vi.fn()} />);
    await waitFor(() => screen.getByRole('button', { name: /use a different account/i }));
    fireEvent.click(screen.getByRole('button', { name: /use a different account/i }));
    const keyInput = await screen.findByPlaceholderText(/api key/i);
    fireEvent.change(keyInput, { target: { value: 'sk-test-abc123' } });
    fireEvent.click(screen.getByRole('button', { name: /^save$/i }));
    await waitFor(() =>
      expect(screen.getByText(/using separate account/i)).toBeInTheDocument(),
    );
  });
});

describe('RepoConfigureModal — no provider guidance (§15 review fix 13)', () => {
  it('shows a "no scheduler configured" message when currentProvider is null', () => {
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider={null} onClose={vi.fn()} />);
    expect(screen.getByText(/no scheduler configured/i)).toBeInTheDocument();
  });

  it('does not show loading indicator when currentProvider is null', () => {
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider={null} onClose={vi.fn()} />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('shows a "Configure" button linking to Default scheduler tab when currentProvider is null', () => {
    render(<RepoConfigureModal repoId="r1" repoName="my-repo" currentProvider={null} onClose={vi.fn()} />);
    expect(screen.getByRole('button', { name: /set up default scheduler/i })).toBeInTheDocument();
  });
});
