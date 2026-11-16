import { useEffect, useState } from 'react';
import type { AgentClient } from '../agent/client';
import { LANES, positionNames, type Lane } from '../agent/positions';
import { useCommand } from '../agent/useCommand';
import type { HonorBallot } from '../bindings/HonorBallot';
import type { HonorCandidate } from '../bindings/HonorCandidate';
import { Button } from './base';
import { ChampionIcon } from './ChampSelectPanel';
import { PositionIcon } from './PositionIcon';

type Props = { ballot: HonorBallot; send: AgentClient['send']; disabled?: boolean };

/**
 * Quanto esperar a cédula trazer o aliado em `honoredPuuids` depois de a LCU
 * aceitar o voto. A LCU responde antes do servidor da Riot, que pode recusar.
 */
export const HONOR_CONFIRM_MS = 10_000;

/** Rota da LCU (`TOP`, `JUNGLE`...) na rota da UI, ou `null` fora do Rift. */
function lane(role: string): Lane | null {
  const name = role.toLowerCase();
  return (LANES as readonly string[]).includes(name) ? name as Lane : null;
}

/**
 * Votação de honra do fim de partida (#182): os aliados da cédula, com um
 * botão para honrar cada um, e "Pular". O voto só é dado como feito quando
 * o aliado aparece em `honoredPuuids` da cédula; se isso não acontece em
 * [`HONOR_CONFIRM_MS`], a tela avisa e deixa tentar de novo. A cédula some
 * sozinha quando a LCU a fecha (volta ao lobby), e a tela some junto.
 */
export function HonorVote({ ballot, send, disabled = false }: Props) {
  const { feedback, pending, run } = useCommand(send);
  const [choice, setChoice] = useState<{ gameId: number; puuid: string | null } | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [unconfirmed, setUnconfirmed] = useState<string | null>(null);
  // Uma cédula de outra partida começa do zero.
  const mine = choice?.gameId === ballot.gameId ? choice : null;
  const honored = ballot.eligibleAllies.find(a => ballot.honoredPuuids.includes(a.puuid));
  const waiting = mine?.puuid && !honored ? mine.puuid : null;

  useEffect(() => {
    if (!waiting) return;
    const timer = setTimeout(() => {
      setUnconfirmed(waiting);
      setChoice(null);
    }, HONOR_CONFIRM_MS);
    return () => clearTimeout(timer);
  }, [waiting]);

  async function honor(ally: HonorCandidate) {
    setTarget(ally.puuid);
    setUnconfirmed(null);
    const ok = await run({ type: 'honorPlayer', puuid: ally.puuid },
      { pending: 'Honrando…', failure: `honrar ${allyName(ally)}` });
    if (ok) setChoice({ gameId: ballot.gameId, puuid: ally.puuid });
  }
  async function skip() {
    setTarget(null);
    setUnconfirmed(null);
    const ok = await run({ type: 'skipHonor' }, { pending: 'Pulando…', failure: 'pular a honra' });
    if (ok) setChoice({ gameId: ballot.gameId, puuid: null });
  }

  if (honored) return <p role="status" className="text-body text-muted">Você honrou {allyName(honored)}.</p>;
  if (mine && mine.puuid === null) return <p role="status" className="text-body text-muted">Você pulou a honra desta partida.</p>;
  if (waiting) return <p role="status" className="text-body text-muted">Voto enviado. Esperando a confirmação da Riot…</p>;
  const notConfirmed = ballot.eligibleAllies.find(a => a.puuid === unconfirmed);

  return (
    <section aria-label="Honra" className="flex w-full max-w-[40rem] flex-col items-center gap-3">
      <div className="text-center">
        <h3 className="text-section text-ink">Honre um aliado</h3>
        <p className="text-caption text-muted">Escolha quem fez a diferença na partida.</p>
      </div>
      <ul className="grid w-full grid-cols-2 gap-2 sm:grid-cols-4">
        {ballot.eligibleAllies.map(ally => {
          const name = allyName(ally);
          const allyLane = lane(ally.role);
          return (
            <li key={ally.puuid}>
              <button type="button" aria-label={`Honrar ${name}`} disabled={disabled || pending}
                onClick={() => { void honor(ally); }}
                className="flex w-full flex-col items-center gap-2 rounded-control border border-control bg-surface p-3 text-center hover:border-accent hover:bg-raised focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-60 disabled:hover:border-control disabled:hover:bg-surface">
                <ChampionIcon id={ally.championId} size={56} />
                <span className="w-full truncate text-body text-ink">{name}</span>
                <span className="flex items-center gap-1 text-caption text-muted">
                  {allyLane && <PositionIcon position={allyLane} size={14} />}
                  {allyLane ? positionNames[allyLane] : ally.championName}
                </span>
                {pending && target === ally.puuid && <span className="text-caption text-muted">Honrando…</span>}
              </button>
            </li>
          );
        })}
      </ul>
      <Button variant="ghost" size="sm" onClick={() => { void skip(); }} disabled={disabled || pending}>
        {pending && target === null ? 'Pulando…' : 'Pular'}
      </Button>
      {notConfirmed && feedback.status !== 'error' && <p role="alert" className="max-w-[24rem] text-center text-caption text-danger">
        A Riot não confirmou o voto em {allyName(notConfirmed)}. Ele pode não ter valido; tente de novo.</p>}
      {feedback.status === 'error' && <p role="alert" className="max-w-[22rem] text-center text-caption text-danger">{feedback.message}</p>}
    </section>
  );
}

function allyName(ally: HonorCandidate): string {
  return ally.bot ? `${ally.championName} (bot)` : ally.summonerName || ally.championName;
}
