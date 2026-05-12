// SPDX-License-Identifier: BUSL-1.1

function djb2(str: string): number {
  let hash = 5381;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) + hash) + str.charCodeAt(i);
  }
  return hash >>> 0;
}

export function deriveOrgColour(id: string): string {
  const h = djb2(id) % 360;
  return `hsl(${h}, 65%, 55%)`;
}
