// SPDX-License-Identifier: BUSL-1.1
// checklist 24.4.9 — Connected accounts section: one row per linked SSO
// provider with a Disconnect action (disabled when it's the last provider),
// plus "Add another account" for the case where the switcher's own "+" icon
// (AccountRail, 24.4.10) isn't visible yet — a single-provider account never
// shows the switcher column.

import { Button, Group, Stack, Text, Tooltip } from '@mantine/core';
import { invoke } from '../ipc/invoke';
import { useProviderAccountsContext } from '../context/ProviderAccountsProvider';
import { startLinkProviderAccountFlow } from '../auth/providerAccountLinking';

async function handleDisconnect(id: string, refresh: () => void) {
  try {
    await invoke('remove_provider_account', { id });
    refresh();
  } catch (e) {
    console.error('[connected-accounts] failed to remove provider account:', e instanceof Error ? e.message : String(e));
  }
}

export default function ConnectedAccountsSection() {
  const { accounts, refresh } = useProviderAccountsContext();
  const isLastProvider = accounts.length <= 1;

  return (
    <Stack gap="sm" className="mb-4">
      <Text size="sm" fw={600}>Connected accounts</Text>
      {accounts.map((account) => (
        <Group key={account.id} justify="space-between">
          <Text size="sm">{account.label ?? account.provider}</Text>
          <Tooltip label="You must keep at least one sign-in method" disabled={!isLastProvider}>
            <Button
              size="xs"
              variant="subtle"
              color="red"
              disabled={isLastProvider}
              onClick={() => handleDisconnect(account.id, refresh)}
            >
              Disconnect
            </Button>
          </Tooltip>
        </Group>
      ))}
      <Button size="xs" variant="light" onClick={startLinkProviderAccountFlow}>
        Add another account
      </Button>
    </Stack>
  );
}
