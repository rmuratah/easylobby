import { playerName } from '../agent/champSelect';
import { swapKindNames, swapsIn, type SwapEntry } from '../agent/swaps';
import type { CommandFeedback, CommandTexts } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { Command } from '../bindings/Command';

type Run = (command: Command, texts: CommandTexts) => Promise<boolean>;
type SwapCommand = 'requestSwap' | 'acceptSwap' | 'declineSwap' | 'cancelSwap';

const buttonClass = 'rounded-control-sm border bg-surface px-2 py-0.5 text-caption font-medium text-ink hover:bg-raised disabled:cursor-not-allowed disabled:opacity-50';

const iconButtonClass = 'flex size-5 shrink-0 items-center justify-center rounded-control-sm border disabled:cursor-not-allowed disabled:opacity-50';

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

function SwapIcon() {
  return (
    <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.25"
      strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
      <path d="M7 4 3 8l4 4M3 8h14M17 12l4 4-4 4M21 16H7" />
    </svg>
  );
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
        // Rótulo curto com o ícone de troca (#623): os botões cabem numa linha do card, que tem altura fixa.
        <button key={`${entry.kind}:${entry.swap.id}`} type="button" disabled={busy}
          className={`${buttonClass} inline-flex items-center gap-1 border-control`}
          aria-label={`Pedir troca de ${swapKindNames[entry.kind]} com ${name}`} title={`Trocar ${swapKindNames[entry.kind]}`}
          onClick={() => sendSwap(run, 'requestSwap', entry)}>
          <SwapIcon />
          <span className="capitalize">{swapKindNames[entry.kind]}</span>
        </button>
      ))}
    </div>
  );
}

/** As trocas com um aliado dentro do card dele (#562): o pedido recebido ou enviado com as respostas e, sem pedido,
 * os botões para pedir. Fica no card para a tela não mudar de formato quando alguém pede uma troca. */
export function AllySwaps({ champSelect, cellId, busy, run }: OffersProps) {
  const received = swapsIn(champSelect, 'RECEIVED', cellId);
  const sent = swapsIn(champSelect, 'SENT', cellId);
  if (received.length === 0 && sent.length === 0) return <SwapOffers champSelect={champSelect} cellId={cellId} busy={busy} run={run} />;
  // Cada pedido numa linha só (#623): os cards dos aliados dividem a altura fixa da coluna.
  return (
    <div className="mt-1 flex flex-col gap-1">
      {received.map(entry => (
        <div key={`r:${entry.kind}:${entry.swap.id}`} role="group" aria-label={`Pedido de troca de ${swapKindNames[entry.kind]}`}
          className="flex min-w-0 items-center gap-1 rounded-control-sm border border-accent bg-accent-soft py-0.5 pr-0.5 pl-1.5 animate-fade-up">
          <p className="min-w-0 flex-1 truncate text-caption font-semibold text-accent-ink" title={`Quer trocar de ${swapKindNames[entry.kind]}`}>
            <span className="sr-only">Quer trocar de {swapKindNames[entry.kind]}</span>
            <span aria-hidden="true">Trocar {swapKindNames[entry.kind]}?</span>
          </p>
          <button type="button" disabled={busy} aria-label="Aceitar" title="Aceitar"
            className={`${iconButtonClass} border-accent bg-accent text-on-accent hover:brightness-110`}
            onClick={() => sendSwap(run, 'acceptSwap', entry)}>
            <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
              strokeLinecap="round" strokeLinejoin="round"><path d="M5 12.5 10 17.5 19 7" /></svg>
          </button>
          <button type="button" disabled={busy} aria-label="Recusar" title="Recusar"
            className={`${iconButtonClass} border-control bg-surface text-ink hover:bg-raised`} onClick={() => sendSwap(run, 'declineSwap', entry)}>
            <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
              strokeLinecap="round"><path d="M6 6l12 12M18 6 6 18" /></svg>
          </button>
        </div>
      ))}
      {sent.map(entry => (
        <div key={`s:${entry.kind}:${entry.swap.id}`} role="group" aria-label={`Seu pedido de troca de ${swapKindNames[entry.kind]}`}
          className="flex min-w-0 items-center gap-1">
          <p className="min-w-0 flex-1 truncate text-caption text-muted" title={`Aguardando a resposta da troca de ${swapKindNames[entry.kind]}.`}>
            Aguardando<span className="sr-only"> a resposta da troca de {swapKindNames[entry.kind]}</span>…
          </p>
          <button type="button" disabled={busy} aria-label="Cancelar pedido" className={`${buttonClass} shrink-0 border-control`}
            onClick={() => sendSwap(run, 'cancelSwap', entry)}>Cancelar</button>
        </div>
      ))}
    </div>
  );
}

/** O andamento e o erro das trocas num aviso flutuante, que não empurra a tela (#562). */
export function SwapFeedbackToast({ feedback }: { feedback: CommandFeedback }) {
  if (feedback.status === 'idle') return null;
  return (
    <p role={feedback.status === 'error' ? 'alert' : 'status'}
      className={`fixed bottom-4 left-1/2 z-40 max-w-[min(32rem,calc(100vw-2rem))] -translate-x-1/2 rounded-card border bg-surface px-4 py-2 text-body shadow-lift animate-fade-up ${feedback.status === 'error' ? 'border-danger text-danger' : 'border-edge text-muted'}`}>
      {feedback.message}
    </p>
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
