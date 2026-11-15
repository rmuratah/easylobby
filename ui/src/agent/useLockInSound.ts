import { useEffect, useRef } from 'react';
import type { ChampSelect } from '../bindings/ChampSelect';
import { myConfirmedActions } from './champSelect';
import { sound } from './sound';

/**
 * Toca o som de confirmação quando um ban ou uma escolha sua fica confirmada
 * na LCU (#498). O que já estava confirmado ao abrir a tela não toca.
 */
export function useLockInSound(champSelect: ChampSelect) {
  const confirmed = myConfirmedActions(champSelect);
  const seen = useRef<Set<number> | null>(null);
  const key = confirmed.map(action => action.id).join(',');
  useEffect(() => {
    if (seen.current === null) {
      seen.current = new Set(confirmed.map(action => action.id));
      return;
    }
    for (const action of confirmed) {
      if (seen.current.has(action.id)) continue;
      seen.current.add(action.id);
      sound.playLockIn(action.type);
    }
    // `confirmed` muda a cada render; `key` diz quando as ações confirmadas mudaram.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
}
