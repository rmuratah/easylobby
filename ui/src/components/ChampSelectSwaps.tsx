import { playerName } from '../agent/champSelect';
import { swapKindNames, swapsIn, type SwapEntry } from '../agent/swaps';
import type { CommandFeedback, CommandTexts } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { Command } from '../bindings/Command';

type Run = (command: Command, texts: CommandTexts) => Promise<boolean>;
type SwapCommand = 'requestSwap' | 'acceptSwap' | 'declineSwap' | 'cancelSwap';

const buttonClass = 'rounded-control-sm border bg-surface px-2 py-0.5 text-caption font-medium text-ink hover:bg-raised disabled:cursor-not-allowed disabled:opacity-50';

function allyName(champSelect: ChampSelect, cellId: number): string {
  const index = champSelect.myTeam.findIndex(p => p.cellId === cellId);
  return index < 0 ? 'Aliado' : playerName(champSelect.myTeam[index], index, true);
}

const texts: Record<SwapCommand, (kind: string) => CommandTexts> = {
  requestSwap: kind => ({ pending: `Pedindo a troca de ${kind}…`, failure: `pedir a troca de ${kind}` }),
  acceptSwap: kind => ({ pending: `Aceitando a troca de ${kind}…`, failure: `aceitar a troca de ${kind}` }),
  declineSwap: kind => ({ pending: `Recusando a troca de ${kind}…`, failure: `recusar a troca de ${kind}` }),
  cancelSwap: kind => ({ pending: `Cancelando a troca de ${kind}…`, failure: `cancelar a troca de ${kind}` }),
};

function sendSwap(run: Run, type: SwapCommand, { kind, swap }: SwapEntry) {
  void run({ type, kind, swapId: swap.id }, texts[type](swapKindNames[kind]));
}

type OffersProps = { champSelect: ChampSelect; cellId: number; busy: boolean; run: Run };

/** Botões para pedir as trocas disponíveis com um aliado (#25). */
export function SwapOffers({ champSelect, cellId, busy, run }: OffersProps) {
  const available = swapsIn(champSelect, 'AVAILABLE', cellId);
  if (available.length === 0) return null;
  const name = allyName(champSelect, cellId);
  return (
    <div className="flex flex-wrap gap-1">
      {available.map(entry => (
        <button key={`${entry.kind}:${entry.swap.id}`} type="button" disabled={busy} className={`${buttonClass} border-control`}
          aria-label={`Pedir troca de ${swapKindNames[entry.kind]} com ${name}`}
          onClick={() => sendSwap(run, 'requestSwap', entry)}>
          Trocar {swapKindNames[entry.kind]}
        </button>
      ))}
    </div>
  );
}

type Props = { champSelect: ChampSelect; busy: boolean; run: Run; feedback: CommandFeedback };

/** Pedidos de troca recebidos e enviados, com as respostas (#25). */
export function ChampSelectSwaps({ champSelect, busy, run, feedback }: Props) {
  const received = swapsIn(champSelect, 'RECEIVED');
  const sent = swapsIn(champSelect, 'SENT');
  if (received.length === 0 && sent.length === 0 && feedback.status === 'idle') return null;
  return (
    <section aria-label="Trocas" className="flex flex-col gap-2 rounded-card border-2 border-accent bg-accent-soft p-3 text-ink">
      {received.map(entry => (
        <div key={`r:${entry.kind}:${entry.swap.id}`} role="group" aria-label={`Pedido de troca de ${swapKindNames[entry.kind]}`}
          className="flex flex-wrap items-center gap-2">
          <p className="font-medium">{allyName(champSelect, entry.swap.cellId)} quer trocar de {swapKindNames[entry.kind]} com você.</p>
          <button type="button" disabled={busy} className={`${buttonClass} border-accent font-semibold`}
            onClick={() => sendSwap(run, 'acceptSwap', entry)}>Aceitar</button>
          <button type="button" disabled={busy} className={`${buttonClass} border-control`}
            onClick={() => sendSwap(run, 'declineSwap', entry)}>Recusar</button>
        </div>
      ))}
      {sent.map(entry => (
        <div key={`s:${entry.kind}:${entry.swap.id}`} role="group" aria-label={`Seu pedido de troca de ${swapKindNames[entry.kind]}`}
          className="flex flex-wrap items-center gap-2">
          <p>Aguardando {allyName(champSelect, entry.swap.cellId)} responder a troca de {swapKindNames[entry.kind]}.</p>
          <button type="button" disabled={busy} className={`${buttonClass} border-control`}
            onClick={() => sendSwap(run, 'cancelSwap', entry)}>Cancelar pedido</button>
        </div>
      ))}
      {feedback.status !== 'idle' && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'}
          className={feedback.status === 'error' ? 'text-danger' : 'text-muted'}>{feedback.message}</p>
      )}
    </section>
  );
}
