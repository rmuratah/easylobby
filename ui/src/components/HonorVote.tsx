import { useEffect, useState, type ReactNode } from 'react';
import type { AgentClient } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { compactNumber, resultQueue, resultTitle } from '../agent/gameResult';
import { formatDuration } from '../agent/profile';
import { LANES, positionNames, type Lane } from '../agent/positions';
import { useCommand } from '../agent/useCommand';
import type { GameQueue } from '../bindings/GameQueue';
import type { GameResult } from '../bindings/GameResult';
import type { HonorBallot } from '../bindings/HonorBallot';
import type { HonorCandidate } from '../bindings/HonorCandidate';
import type { MatchPlayer } from '../bindings/MatchPlayer';
import { Button } from './base';
import { cx } from './base/cx';
import { ChampionIcon } from './ChampSelectPanel';
import { PositionIcon } from './PositionIcon';
import { ballotPlayerStats, playerName, votesLeft } from '../agent/honor';

type Props = {
  ballot: HonorBallot;
  send: AgentClient['send'];
  disabled?: boolean;
  /**
   * Resultado da partida, que chega antes da cédula (#629): mostra a faixa
   * de resultado e o desempenho de cada jogador. Sem ele, só a votação.
   */
  result?: GameResult | null;
  /** Filas conhecidas, para o nome da fila na faixa de resultado. */
  queues?: GameQueue[];
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
 * pula. Com o resultado da partida, mostra também como ela terminou e o
 * desempenho de cada jogador, com a arte do campeão no fundo do card (#629).
 */
export function HonorVote({ ballot, send, disabled = false, result = null, queues = [], onDone }: Props) {
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
    const stats = ballotPlayerStats(result, ballot, player);
    const status = honored ? 'Honrado' : waiting === player.puuid ? 'Confirmando…' : pending && target === player.puuid ? 'Honrando…' : null;
    return (
      <li key={player.puuid} className="w-[calc(50%-0.25rem)] sm:w-[calc(20%-0.4rem)]">
        <button type="button" aria-label={honored ? `${name}, honrado` : `Honrar ${name}`} aria-pressed={honored}
          disabled={busy || honored || left === 0}
          onClick={() => { void honor(player); }}
          className={cx('group relative isolate flex w-full flex-col items-center gap-1.5 overflow-hidden rounded-control border border-t-4 bg-surface p-3 text-center focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
            opponent ? 'border-t-danger' : 'border-t-status-game',
            honored ? 'border-accent bg-raised' : 'border-control hover:border-accent hover:bg-raised disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:border-control disabled:hover:bg-surface')}>
          <ChampionArt championId={player.championId} />
          <ChampionIcon id={player.championId} size={48} />
          <span className="w-full truncate text-body text-ink">{name}</span>
          <span className="flex items-center gap-1 text-caption text-muted">
            {playerLane && <PositionIcon position={playerLane} size={14} />}
            {playerLane ? positionNames[playerLane] : player.championName}
          </span>
          {stats && <Performance player={stats} />}
          {/* No canto, por cima da arte, para não reservar uma linha vazia no card. */}
          {status && <span className={cx('absolute right-1.5 top-1.5 rounded-full bg-surface px-2 py-0.5 text-caption shadow-soft',
            honored ? 'font-semibold text-accent' : 'text-muted')}>{status}</span>}
        </button>
      </li>
    );
  };

  return (
    <section aria-label="Honra" className="my-auto flex w-full max-w-[52rem] flex-col items-center gap-4">
      {result && result.gameId === ballot.gameId && <ResultBanner result={result} queues={queues} />}
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

/**
 * A splash da skin padrão do campeão no fundo do card, apagada por baixo de
 * um degradê da cor do card para o nome e os números continuarem legíveis.
 * Some se a imagem não carregar.
 */
function ChampionArt({ championId }: { championId: number }) {
  const [failed, setFailed] = useState(false);
  if (failed || championId <= 0) return null;
  return (
    <>
      <img src={assetUrl('skinSplash', championId * 1000)} alt="" aria-hidden="true" loading="lazy"
        className="absolute inset-0 -z-10 h-full w-full object-cover object-[center_20%] opacity-45 transition-opacity group-hover:opacity-60"
        onError={() => setFailed(true)} />
      <span aria-hidden="true" className="absolute inset-0 -z-10 bg-gradient-to-t from-surface via-surface/85 to-surface/30" />
    </>
  );
}

/** KDA, dano a campeões e ouro do jogador na partida. */
function Performance({ player: p }: { player: MatchPlayer }) {
  return (
    <dl className="flex w-full flex-col items-center gap-0.5">
      <dt className="sr-only">KDA</dt>
      <dd className="text-body font-semibold tabular-nums text-ink">{p.kills} / {p.deaths} / {p.assists}</dd>
      <dt className="sr-only">Dano a campeões</dt>
      <dd className="text-caption tabular-nums text-muted">{compactNumber(p.damageToChampions)} de dano</dd>
      <dt className="sr-only">Ouro</dt>
      <dd className="text-caption tabular-nums text-muted">{compactNumber(p.gold)} de ouro</dd>
    </dl>
  );
}

/** Como a partida terminou: resultado, fila e duração, acima da votação. */
function ResultBanner({ result, queues }: { result: GameResult; queues: GameQueue[] }) {
  const tone = result.result === 'win' ? 'text-accent-ink' : result.result === 'loss' ? 'text-danger' : 'text-muted';
  return (
    <p aria-label="Resultado da partida" className="flex flex-wrap items-center justify-center gap-x-2 rounded-control bg-raised px-4 py-1.5 text-body text-muted">
      <span className={cx('font-semibold', tone)}>{resultTitle[result.result]}</span>
      <span aria-hidden="true">·</span>
      <span>{resultQueue(result, queues)}</span>
      <span aria-hidden="true">·</span>
      <span className="tabular-nums">{formatDuration(result.durationSecs)}</span>
    </p>
  );
}
