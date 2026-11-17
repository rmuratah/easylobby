import { useEffect, useState } from 'react';
import type { LolUpdate } from '../bindings/LolUpdate';
import type { LolUpdateStage } from '../bindings/LolUpdateStage';
import { Spinner } from './base';

/** Quanto tempo o aviso de "atualização concluída" fica na tela. */
export const DONE_NOTICE_MS = 8000;

const titles: Record<LolUpdateStage, string> = {
  checking: 'Preparando a atualização do League of Legends',
  downloading: 'Atualizando o League of Legends',
  repairing: 'Reparando o League of Legends',
  pending: 'Atualização do League of Legends disponível',
};

function description(update: LolUpdate): string {
  switch (update.stage) {
    case 'checking': return 'O LoL está conferindo os arquivos do jogo antes de baixar o patch novo.';
    case 'downloading': return 'O LoL está baixando e aplicando o patch novo.';
    case 'repairing': return 'O LoL está corrigindo arquivos do jogo.';
    case 'pending': return 'O LoL baixa o patch novo quando você sair do grupo e da partida, como no client oficial.';
  }
}

/** Se o LoL está mexendo nos arquivos do jogo (o agente não fecha a UI oficial nesse meio-tempo). */
const running = (update: LolUpdate | null) =>
  !!update && (update.stage === 'downloading' || update.stage === 'repairing' || update.blocksQueue);

/**
 * Patch do LoL em andamento (#575): com a UI oficial fechada, o jogador não via
 * o progresso e achava que o app tinha travado. Mostra a etapa, a barra de
 * progresso e se a fila está esperando; quando termina, avisa por alguns
 * segundos que a fila já liberou.
 */
export function LolUpdateBanner({ update }: { update: LolUpdate | null }) {
  const [previous, setPrevious] = useState(update);
  const [done, setDone] = useState(false);
  // Ajuste durante a renderização (e não num efeito): o fim do patch é a passagem de "rodando" para "sem patch".
  if (previous !== update) {
    setPrevious(update);
    if (running(previous) && !update) setDone(true);
    else if (update) setDone(false);
  }
  useEffect(() => {
    if (!done) return;
    const timer = setTimeout(() => setDone(false), DONE_NOTICE_MS);
    return () => clearTimeout(timer);
  }, [done]);

  if (!update) {
    if (!done) return null;
    return (
      <section role="status" aria-label="Atualização do League of Legends"
        className="col-span-full flex flex-col gap-1 rounded-card border border-line bg-surface card p-4">
        <h2 className="text-body font-semibold text-ink">Atualização do League of Legends concluída</h2>
        <p className="text-body text-muted">A busca de partida já está liberada.</p>
      </section>
    );
  }
  const percent = update.percent;
  return (
    <section role="status" aria-label="Atualização do League of Legends"
      className="col-span-full flex flex-col gap-3 rounded-card border border-warn bg-surface card p-4">
      <div className="flex items-start gap-3">
        {update.stage !== 'pending' && <span className="shrink-0 text-accent"><Spinner size={20} label={null} /></span>}
        <div className="flex min-w-0 flex-col gap-1">
          <h2 className="text-body font-semibold text-ink">
            {titles[update.stage]}{percent !== null && <span className="tabular-nums">: {percent}%</span>}
          </h2>
          <p className="text-body text-muted">
            {description(update)}{' '}
            {update.blocksQueue
              ? 'A busca de partida fica travada até o patch terminar e volta sozinha depois.'
              : update.stage !== 'pending' && 'Você pode continuar usando o app.'}
          </p>
        </div>
      </div>
      {percent !== null && (
        <div role="progressbar" aria-label="Progresso da atualização" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}
          className="h-1.5 overflow-hidden rounded-full bg-raised">
          <div className="h-full rounded-full bg-accent transition-[width] duration-500" style={{ width: `${percent}%` }} />
        </div>
      )}
    </section>
  );
}
