import { useEffect, useRef } from 'react';
import type { AgentState } from '../bindings/AgentState';
import type { ConnectionSnapshot } from './client';
import { sound } from './sound';
import { soundCues } from './soundEvents';

/** Intervalo entre dois sons da mesma mudança, para não tocarem por cima. */
const CUE_GAP_MS = 180;

/**
 * Toca os sons dos momentos do lobby, da fila, da seleção e das conversas
 * (#501), comparando cada estado novo do agente com o anterior. Também
 * libera o áudio no primeiro clique ou tecla, como o navegador exige.
 */
export function useAppSounds(connection: ConnectionSnapshot, openConversationId: string | null) {
  const state = connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state : null;
  const previous = useRef<AgentState | null>(null);
  const openRef = useRef(openConversationId);
  useEffect(() => { openRef.current = openConversationId; }, [openConversationId]);

  useEffect(() => {
    const unlock = () => sound.unlock();
    window.addEventListener('pointerdown', unlock, { once: true });
    window.addEventListener('keydown', unlock, { once: true });
    return () => {
      window.removeEventListener('pointerdown', unlock);
      window.removeEventListener('keydown', unlock);
    };
  }, []);

  useEffect(() => {
    const prev = previous.current;
    previous.current = state;
    if (state === prev) return;
    const focused = typeof document !== 'undefined' && document.hasFocus();
    const cues = soundCues(prev, state, { focused, openConversationId: openRef.current });
    if (cues.length === 0) return;
    const [first, ...rest] = cues;
    sound.play(first);
    // Sem cancelar numa próxima mudança: o estado muda várias vezes por segundo na seleção.
    rest.forEach((cue, index) => setTimeout(() => sound.play(cue), CUE_GAP_MS * (index + 1)));
  }, [state]);
}
