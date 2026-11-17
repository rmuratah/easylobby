import { useEffect, useState, type ReactNode } from 'react';
import type { AgentClient } from '../agent/client';
import { LANES, positionNames, type Lane } from '../agent/positions';
import { useCommand } from '../agent/useCommand';
import type { HonorBallot } from '../bindings/HonorBallot';
import type { HonorCandidate } from '../bindings/HonorCandidate';
import { Button } from './base';
import { cx } from './base/cx';
import { ChampionIcon } from './ChampSelectPanel';
import { PositionIcon } from './PositionIcon';
import { playerName, votesLeft } from '../agent/honor';

type Props = {
  ballot: HonorBallot;
  send: AgentClient['send'];
  disabled?: boolean;
  /** O jogador encerrou a votação com "Pular" ou "Concluir". */
  onDone: () => void;
};

/**
 * Quanto esperar a cédula trazer o jogador em `honoredPuuids` depois de a LCU
 * aceitar o voto. A LCU responde antes do servidor da Riot, que pode recusar.
 */
export const HONOR_CONFIRM_MS = 10_000;

/** Rota da LCU (`TOP`, `JUNGLE`...) na rota da UI, ou `null` fora do Rift. */
function lane(role: string): Lane | null {
  const name = role.toLowerCase();
  return (LANES as readonly string[]).includes(name) ? name as Lane : null;
}

/**
 * Votação de honra do fim de partida (#182), como no client oficial: as
 * honras da partida (que mudam de partida para partida), os aliados em cima e
 * os adversários embaixo, um voto por jogador. Um voto só é dado como feito
 * quando o jogador aparece em `honoredPuuids` da cédula; se isso não
 * acontece em [`HONOR_CONFIRM_MS`], a tela avisa e deixa tentar de novo.
 * Quem chama tira a votação da tela quando as honras acabam ou o jogador
 * pula.
 */
export function HonorVote({ ballot, send, disabled = false, onDone }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const [sent, setSent] = useState<{ gameId: number; puuid: string } | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [unconfirmed, setUnconfirmed] = useState<string | null>(null);
  const players = [...ballot.eligibleAllies, ...ballot.eligibleOpponents];
  // Um voto de outra partida, ou já confirmado, não espera mais nada.
  const waiting = sent && sent.gameId === ballot.gameId && !ballot.honoredPuuids.includes(sent.puuid) ? sent.puuid : null;
  const left = votesLeft(ballot);

  useEffect(() => {
    if (!waiting) return;
    const timer = setTimeout(() => {
      setUnconfirmed(waiting);
      setSent(null);
    }, HONOR_CONFIRM_MS);
    return () => clearTimeout(timer);
  }, [waiting]);

  async function honor(player: HonorCandidate) {
    setTarget(player.puuid);
    setUnconfirmed(null);
    const ok = await run({ type: 'honorPlayer', puuid: player.puuid },
      { pending: 'Honrando…', failure: `honrar ${playerName(player)}` });
    if (ok) setSent({ gameId: ballot.gameId, puuid: player.puuid });
  }
  async function finish() {
    setTarget(null);
    // O agente já encerra a tela da honra sozinho com a UI oficial fechada,
    // então uma falha aqui não prende o jogador na votação.
    await run({ type: 'skipHonor' }, { pending: 'Encerrando…', failure: 'encerrar a honra' });
    onDone();
  }

  const busy = disabled || pending || waiting !== null;
  const notConfirmed = players.find(p => p.puuid === unconfirmed);
  const card = (player: HonorCandidate, opponent: boolean) => {
    const name = playerName(player);
    const playerLane = lane(player.role);
    const honored = ballot.honoredPuuids.includes(player.puuid);
    const status = honored ? 'Honrado' : waiting === player.puuid ? 'Confirmando…' : pending && target === player.puuid ? 'Honrando…' : null;
    return (
      <li key={player.puuid} className="w-[calc(50%-0.25rem)] sm:w-[calc(20%-0.4rem)]">
        <button type="button" aria-label={honored ? `${name}, honrado` : `Honrar ${name}`} aria-pressed={honored}
          disabled={busy || honored || left === 0}
          onClick={() => { void honor(player); }}
          className={cx('flex w-full flex-col items-center gap-2 rounded-control border border-t-4 bg-surface p-3 text-center focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
            opponent ? 'border-t-danger' : 'border-t-status-game',
            honored ? 'border-accent bg-raised' : 'border-control hover:border-accent hover:bg-raised disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:border-control disabled:hover:bg-surface')}>
          <ChampionIcon id={player.championId} size={48} />
          <span className="w-full truncate text-body text-ink">{name}</span>
          <span className="flex items-center gap-1 text-caption text-muted">
            {playerLane && <PositionIcon position={playerLane} size={14} />}
            {playerLane ? positionNames[playerLane] : player.championName}
          </span>
          <span className={cx('min-h-[1lh] text-caption', honored ? 'font-semibold text-accent' : 'text-muted')}>{status}</span>
        </button>
      </li>
    );
  };

  return (
    <section aria-label="Honra" className="flex w-full max-w-[52rem] flex-col items-center gap-4">
      <div className="flex flex-col items-center gap-2 text-center">
        <h2 className="text-[1.375rem] font-semibold tracking-[-0.01em] text-ink">Honre jogadores</h2>
        <Diamonds total={ballot.votes} left={left} />
        <p className="text-caption text-muted">Escolha quem fez a diferença na partida, do seu time ou do time inimigo.</p>
      </div>
      <Team label="Seu time" tone="text-status-game">{ballot.eligibleAllies.map(p => card(p, false))}</Team>
      {ballot.eligibleOpponents.length > 0 &&
        <Team label="Time inimigo" tone="text-danger">{ballot.eligibleOpponents.map(p => card(p, true))}</Team>}
      {waiting && <p role="status" className="text-caption text-muted">Voto enviado. Esperando a confirmação da Riot…</p>}
      <Button variant={ballot.honoredPuuids.length > 0 ? 'secondary' : 'ghost'} size="sm"
        onClick={() => { void finish(); }} disabled={disabled || pending || waiting !== null}>
        {pending && target === null ? 'Encerrando…' : ballot.honoredPuuids.length > 0 ? 'Concluir' : 'Pular'}
      </Button>
      {notConfirmed && feedback.status !== 'error' && <p role="alert" className="max-w-[24rem] text-center text-caption text-danger">
        A Riot não confirmou o voto em {playerName(notConfirmed)}. Ele pode não ter valido; tente de novo.</p>}
      {feedback.status === 'error' && <p role="alert" className="max-w-[22rem] text-center text-caption text-danger">{feedback.message}</p>}
    </section>
  );
}

function Team({ label, tone, children }: { label: string; tone: string; children: ReactNode }) {
  return (
    <div className="flex w-full flex-col gap-2">
      <h3 className={cx('text-center text-caption font-semibold uppercase tracking-[0.06em]', tone)}>{label}</h3>
      <ul aria-label={label} className="flex w-full flex-wrap justify-center gap-2">{children}</ul>
    </div>
  );
}

/** Uma marca por honra da partida: cheia para as que sobram, vazia para as usadas. */
function Diamonds({ total, left }: { total: number; left: number }) {
  const label = total === 0 ? 'Sem honras nesta partida'
    : `${left} de ${total} ${total === 1 ? 'honra restante' : 'honras restantes'}`;
  return (
    <div role="img" aria-label={label} className="flex items-center gap-2">
      {Array.from({ length: total }, (_, i) => (
        <svg key={i} width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"
          className={i < left ? 'text-accent' : 'text-muted'}>
          <path d="M8 1 15 8 8 15 1 8Z" fill={i < left ? 'currentColor' : 'none'} stroke="currentColor" strokeWidth="1.5" />
        </svg>
      ))}
    </div>
  );
}
