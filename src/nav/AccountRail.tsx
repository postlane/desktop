// SPDX-License-Identifier: BUSL-1.1
//
// v2.0 §1 "Provider account switcher" / checklist 24.4.10. Far-left icon
// column, rendered only once the authenticated user has 2+ connected
// provider accounts -- a single-connected-account user (the common case)
// never sees this column, so it adds no chrome until a second account is
// actually connected.

import { ActionIcon, Menu, Stack, Tooltip } from '@mantine/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '../ipc/invoke';
import { useProviderAccountsContext } from '../context/ProviderAccountsProvider';
import type { ProviderAccountsState } from '../hooks/useProviderAccounts';
import { deriveOrgColour } from '../formatting/orgColour';

interface AccountIconProps {
  account: ProviderAccountsState['accounts'][number];
  active: boolean;
  onSwitch: (_id: string) => void;
  onRemoved: () => void;
}

function accountLabel(account: AccountIconProps['account']): string {
  return account.label ?? account.provider;
}

async function handleRemove(id: string, onRemoved: () => void) {
  try {
    await invoke('remove_provider_account', { id });
    onRemoved();
  } catch (e) {
    console.error('[account-rail] failed to remove provider account:', e instanceof Error ? e.message : String(e));
  }
}

function AccountIcon({ account, active, onSwitch, onRemoved }: AccountIconProps) {
  const label = accountLabel(account);
  const initial = label[0]?.toUpperCase() ?? '?';
  return (
    <Stack gap={4} align="center">
      <Tooltip label={label} position="right">
        <ActionIcon
          radius="xl"
          size={36}
          variant={active ? 'filled' : 'light'}
          color={deriveOrgColour(account.id)}
          aria-label={label}
          aria-pressed={active}
          onClick={() => onSwitch(account.id)}
        >
          {initial}
        </ActionIcon>
      </Tooltip>
      <Menu withinPortal position="right-start">
        <Menu.Target>
          <ActionIcon variant="subtle" size={16} aria-label={`Options for ${label}`}>
            ⋮
          </ActionIcon>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Item
            color="red"
            disabled={account.is_primary}
            onClick={() => handleRemove(account.id, onRemoved)}
          >
            Remove
          </Menu.Item>
        </Menu.Dropdown>
      </Menu>
    </Stack>
  );
}

async function handleAddAccount() {
  try {
    const port = await invoke<number>('get_local_server_port');
    openUrl(`https://postlane.dev/login?desktop=1&port=${port}&mode=link_provider_account`).catch(console.error);
  } catch (e) {
    console.error('[account-rail] get_local_server_port failed — opening without port:', e);
    openUrl('https://postlane.dev/login?desktop=1&mode=link_provider_account').catch(console.error);
  }
}

export default function AccountRail({ onSwitch }: { onSwitch: (_id: string) => void }) {
  const { accounts, activeAccountId, refresh } = useProviderAccountsContext();

  if (accounts.length < 2) return null;

  return (
    <div
      role="navigation"
      aria-label="Provider accounts"
      className="has-background-white"
      style={{ width: 56, height: '100vh', display: 'flex', flexDirection: 'column', alignItems: 'center',
        paddingBlock: '0.75rem', gap: '0.75rem', borderRight: '1px solid var(--bulma-border-weak)' }}
    >
      {accounts.map((account) => (
        <AccountIcon
          key={account.id}
          account={account}
          active={account.id === activeAccountId}
          onSwitch={onSwitch}
          onRemoved={refresh}
        />
      ))}
      <Tooltip label="Add provider account" position="right">
        <ActionIcon radius="xl" size={36} variant="light" aria-label="Add provider account" onClick={handleAddAccount}>
          +
        </ActionIcon>
      </Tooltip>
    </div>
  );
}
