import { useState } from 'react';
import type { AgentClient } from '../agent/client';
import { useCommand } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import { Button } from './base';

/**
 * Botão Sair da seleção de campeões, só na personalizada (#488): como no
 * client oficial, todos voltam para o lobby da personalizada. Nas filas
 * normais sair seria esquiva (com penalidade), e o botão não aparece. Pede
 * confirmação antes, porque leva todo mundo de volta para o lobby.
 */
export function QuitCustomChampSelect({ champSelect, send }: { champSelect: ChampSelect; send: AgentClient['send'] }) {
  const quit = useCommand(send);
  const [confirming, setConfirming] = useState(false);
  if (!champSelect.isCustomGame) return null;
  const error = quit.feedback.status === 'error' ? quit.feedback.message : null;
  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      {confirming ? (
        <>
          <p className="text-caption text-muted">Sair da seleção? Todos voltam para o lobby.</p>
          <Button size="sm" variant="danger" loading={quit.pending}
            onClick={() => void quit.run({ type: 'quitCustomChampSelect' }, { pending: '', failure: 'sair da seleção' })
              .then(ok => { if (!ok) setConfirming(false); })}>
            Sair da seleção
          </Button>
          <Button size="sm" variant="ghost" disabled={quit.pending} onClick={() => setConfirming(false)}>Ficar</Button>
        </>
      ) : (
        <Button size="sm" onClick={() => setConfirming(true)}>Sair</Button>
      )}
      {error && <p role="alert" className="basis-full text-right text-caption text-danger">{error}</p>}
    </div>
  );
}
