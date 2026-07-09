// SPDX-License-Identifier: BUSL-1.1
// Shared by the switcher's "+" icon (AccountRail) and Settings -- Account's
// "Add another account" button (24.4.9) -- both start the identical
// mode=link_provider_account OAuth flow.

import { openUrl } from '@tauri-apps/plugin-opener';
import { invoke } from '../ipc/invoke';

export function startLinkProviderAccountFlow(): void {
  invoke<number>('get_local_server_port')
    .then((port) => {
      openUrl(`https://postlane.dev/login?desktop=1&port=${port}&mode=link_provider_account`).catch(console.error);
    })
    .catch((e) => {
      console.error('[provider-account-linking] get_local_server_port failed — opening without port:', e);
      openUrl('https://postlane.dev/login?desktop=1&mode=link_provider_account').catch(console.error);
    });
}
