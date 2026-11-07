import { useEffect, useRef, useState } from 'react';
import type { AgentClient } from './client';
import { friendDisplayName } from './friends';
import { freeSlots, invitableFriends, isOpenInvitation } from './invitations';
import type { AgentState } from '../bindings/AgentState';
import type { Friend } from '../bindings/Friend';

/** Tipo do arrasto de um amigo da lista de amigos para o lobby (#206); leva o `puuid`. */
export const FRIEND_DRAG_TYPE = 'application/x-easylobby-friend';

/** Quem convidar: um amigo, um jogador recente ou achado pelo nome. */
export type InviteTarget = { puuid: string; summonerId: number; name: string };

export type LobbyInvite = {
  /** Se dá para convidar agora: lobby aberto a convites, na fase de lobby e com vaga. */
  canInvite: boolean;
  /** Amigos que dá para convidar agora, sem os que acabaram de ser convidados. */
  candidates: Friend[];
  /** Vagas que ainda dá para preencher com convites. */
  slots: number;
  /** Convite em envio para este amigo. */
  pending: (puuid: string) => boolean;
  invite: (friend: Friend) => Promise<void>;
  /** Convida vários jogadores de uma vez (pop-up do "+"). Devolve se o agente aceitou. */
  inviteMany: (targets: InviteTarget[]) => Promise<boolean>;
  /** Convida pelo Riot ID, amigo ou não ("Convidar por nome"). Devolve se o agente aceitou. */
  inviteByRiotId: (gameName: string, tagLine: string) => Promise<boolean>;
  /** Se `puuid` já está no grupo, foi convidado ou tem convite em envio. */
  taken: (puuid: string) => boolean;
  /** Convida pelo `puuid` (amigo arrastado); explica quando ele não pode ser convidado. */
  inviteByPuuid: (puuid: string) => void;
  error: string;
};

/**
 * Convites que o jogador manda do lobby (#32, #206): pelo "+" da vaga livre e
 * arrastando um amigo. Um convite aceito pelo agente fica guardado até o
 * lobby mostrar o convite ou o amigo, para ele não aparecer de novo na lista.
 */
export function useLobbyInvite(state: AgentState, send: AgentClient['send']): LobbyInvite {
  const { lobby, friends, matchmaking, gameflowPhase: phase } = state;
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  const [done, setDone] = useState<ReadonlySet<string>>(new Set());
  const [error, setError] = useState('');
  const inFlight = useRef(new Set<string>());
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const reflected = (puuid: string) => !lobby
    || lobby.invitations.some(i => i.puuid === puuid) || lobby.members.some(m => m.puuid === puuid);
  if ([...done].some(reflected)) setDone(new Set([...done].filter(puuid => !reflected(puuid))));

  const slots = lobby ? freeSlots(lobby) - done.size : 0;
  const canInvite = !!lobby && lobby.canInvite && phase === 'Lobby' && !matchmaking && !state.readyCheck && slots > 0;
  const candidates = canInvite && lobby ? invitableFriends(lobby, friends).filter(f => !done.has(f.puuid)) : [];

  async function inviteMany(selected: InviteTarget[]): Promise<boolean> {
    const batch = selected.filter(f => !inFlight.current.has(f.puuid) && !done.has(f.puuid));
    if (batch.length === 0) return false;
    const puuids = batch.map(f => f.puuid);
    for (const puuid of puuids) inFlight.current.add(puuid);
    setPending(current => new Set([...current, ...puuids]));
    setError('');
    const names = batch.map(t => t.name).join(', ');
    try {
      // A LCU só entrega o convite com o `summonerId`; o `puuid` vai junto, como no client oficial.
      const result = await send({ type: 'inviteToLobby', invitees: batch.map(f => ({ summonerId: f.summonerId, puuid: f.puuid })) });
      if (!mounted.current) return false;
      if (result.status === 'ok') {
        setDone(current => new Set([...current, ...puuids]));
        return true;
      }
      setError(result.code === 'notConnected'
        ? 'O League of Legends desconectou. Aguarde a reconexão e confira o grupo antes de convidar de novo.'
        : `Não foi possível convidar ${names}: ${result.message}`);
    } catch {
      if (mounted.current) setError('Não foi possível confirmar o convite. Confira a conexão e o grupo antes de tentar de novo.');
    } finally {
      for (const puuid of puuids) inFlight.current.delete(puuid);
      if (mounted.current) {
        setPending(current => new Set([...current].filter(puuid => !puuids.includes(puuid))));
      }
    }
    return false;
  }

  async function invite(friend: Friend) {
    await inviteMany([friendTarget(friend)]);
  }

  async function inviteByRiotId(gameName: string, tagLine: string): Promise<boolean> {
    const riotId = `${gameName.trim()}#${tagLine.trim().replace(/^#/, '')}`;
    setError('');
    try {
      const result = await send({ type: 'inviteByRiotId', gameName, tagLine });
      if (!mounted.current) return false;
      if (result.status === 'ok') return true;
      setError(result.code === 'notFound' ? `Nenhum jogador encontrado com o Riot ID ${riotId}.`
        : result.code === 'notConnected' ? 'O League of Legends desconectou. Aguarde a reconexão e confira o grupo antes de convidar de novo.'
        : `Não foi possível convidar ${riotId}: ${result.message}`);
    } catch {
      if (mounted.current) setError('Não foi possível confirmar o convite. Confira a conexão e o grupo antes de tentar de novo.');
    }
    return false;
  }

  const taken = (puuid: string) => pending.has(puuid) || done.has(puuid) || !!lobby && (lobby.members.some(m => m.puuid === puuid)
    || lobby.invitations.some(i => i.puuid === puuid && isOpenInvitation(i.state)));

  function inviteByPuuid(puuid: string) {
    const friend = friends.find(f => f.puuid === puuid);
    if (!friend) return;
    if (candidates.some(c => c.puuid === puuid)) {
      void invite(friend);
      return;
    }
    const name = friendDisplayName(friend);
    setError(!canInvite ? `Não dá para convidar agora: ${lobby && !lobby.canInvite ? 'só o líder convida neste grupo' : 'o grupo está cheio ou fora da fase de lobby'}.`
      : lobby?.members.some(m => m.puuid === puuid) ? `${name} já está no grupo.`
      : lobby?.invitations.some(i => i.puuid === puuid && isOpenInvitation(i.state)) || done.has(puuid) ? `${name} já foi convidado.`
      : `${name} não pode ser convidado agora (offline, no celular ou sem LoL).`);
  }

  return { canInvite, candidates, slots: Math.max(slots, 0), pending: puuid => pending.has(puuid) || done.has(puuid), invite, inviteMany, inviteByRiotId, taken, inviteByPuuid, error };
}

/** O alvo de convite de um amigo. */
export function friendTarget(friend: Friend): InviteTarget {
  return { puuid: friend.puuid, summonerId: friend.summonerId, name: friendDisplayName(friend) };
}
