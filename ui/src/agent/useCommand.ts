import { useEffect, useRef, useState } from 'react';
import type { AgentClient } from './client';
import type { Command } from '../bindings/Command';
import { useDismiss } from './useDismiss';

export type CommandFeedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
export type CommandTexts = { pending: string; success?: string; failure: string };

/**
 * Manda comandos ao agente e guarda o retorno para a tela. Respostas que
 * chegam depois de o componente sair (seleção nova, queda) são ignoradas.
 */
export function useCommand(send: AgentClient['send']) {
  const [feedback, setFeedback] = useState<CommandFeedback>({ status: 'idle', message: '' });
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  // O erro some sozinho depois de alguns segundos (#335).
  useDismiss(feedback.status === 'error', feedback.message, () => setFeedback({ status: 'idle', message: '' }));
  async function run(command: Command, texts: CommandTexts): Promise<boolean> {
    setFeedback({ status: 'pending', message: texts.pending });
    try {
      const result = await send(command);
      if (!mounted.current) return false;
      if (result.status === 'error') {
        setFeedback({ status: 'error', message: `Não foi possível ${texts.failure}: ${result.message}` });
        return false;
      }
      setFeedback({ status: texts.success ? 'success' : 'idle', message: texts.success ?? '' });
      return true;
    } catch (error) {
      if (!mounted.current) return false;
      // Timeout ou falha de envio não provam que a LCU deixou de executar o comando.
      const reason = error instanceof Error ? error.message : 'erro desconhecido';
      setFeedback({ status: 'error', message: `Não foi possível confirmar: ${reason} Confira o estado na tela.` });
      return false;
    }
  }
  return { feedback, pending: feedback.status === 'pending', run };
}
