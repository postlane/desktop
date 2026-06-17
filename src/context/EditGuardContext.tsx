// SPDX-License-Identifier: BUSL-1.1

import { createContext, useContext } from 'react';
import type { ViewSelection } from '../types';

export interface EditGuardContextValue {
  resetSignal: number;
  setDirty: (_dirty: boolean) => void;
  pendingNavSel: ViewSelection | null;
  onNavCancelled: () => void;
}

export const EditGuardContext = createContext<EditGuardContextValue | null>(null);

export function useEditGuard(): EditGuardContextValue {
  const ctx = useContext(EditGuardContext);
  if (ctx === null) {
    throw new Error('useEditGuard must be used within EditGuardContext.Provider');
  }
  return ctx;
}
