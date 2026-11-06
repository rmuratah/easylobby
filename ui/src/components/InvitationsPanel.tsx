import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { friendDisplayName } from '../agent/friends';
import {
  freeSlots, invitableFriends, invitationQueueName, invitationStateNames, inviteeName, inviterName,
} from '../agent/invitations';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import type { Friend } from '../bindings/Friend';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import { Avatar, Button, Card } from './base';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type InvitationCommand = Extract<Command, { type: 'acceptInvitation' | 'declineInvitation' | 'inviteToLobby' }>;
type Feedback = { status: 'idle' | 'success' | 'error'; message: string };
const subheading = 'text-caption font-semibold uppercase tracking-[0.06em] text-muted';

/**
 * Convites de lobby (#32): os convites recebidos, com aceitar e recusar, e,
 * dentro de um lobby, os amigos para convidar e os convites que o grupo mandou.
 */
export function InvitationsPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <Card as="section" aria-label="Convites" className="flex w-full flex-col gap-2 p-4 text-ink">
        <h2 className="text-section font-semibold">Convites</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends para ver os convites.</p>
      </Card>
    );
  }
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <InvitationsActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} />;
}

/** Chave do comando em andamento, para não repetir o mesmo clique. */
function commandKey(command: InvitationCommand): string {
  return command.type === 'inviteToLobby' ? `invite:${command.invitees[0]?.puuid}` : `invitation:${command.invitationId}`;
}

function InvitationsActions({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const { lobby, friends, receivedInvitations: received, matchmaking, gameflowPhase: phase } = state;
  const [pending, setPending] = useState<ReadonlySet<string>>(new Set());
  // Comandos aceitos pelo agente cujo evento ainda não chegou: o botão fica
  // desligado até o convite sair da lista (ou aparecer no lobby).
  const [done, setDone] = useState<ReadonlySet<string>>(new Set());
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(new Set<string>());
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const reflected = (key: string) => key.startsWith('invite:')
    ? !lobby || lobby.invitations.some(i => `invite:${i.puuid}` === key) || lobby.members.some(m => `invite:${m.puuid}` === key)
    : !received.some(i => `invitation:${i.invitationId}` === key);
  if ([...done].some(reflected)) setDone(new Set([...done].filter(key => !reflected(key))));

  const busy = (key: string) => pending.has(key) || done.has(key);
  const canInvite = !!lobby && lobby.canInvite && phase === 'Lobby' && !matchmaking && !state.readyCheck;
  const slots = lobby ? freeSlots(lobby) - [...done].filter(key => key.startsWith('invite:')).length : 0;
  const candidates = lobby ? invitableFriends(lobby, friends).filter(f => !done.has(`invite:${f.puuid}`)) : [];

  async function execute(command: InvitationCommand, success: string) {
    const key = commandKey(command);
    if (inFlight.current.has(key) || done.has(key)) return;
    inFlight.current.add(key);
    setPending(current => new Set(current).add(key));
    try {
      const result = await send(command);
      if (!mounted.current) return;
      if (result.status === 'ok') {
        setDone(current => new Set(current).add(key));
        setFeedback({ status: 'success', message: success });
      } else {
        setFeedback({ status: 'error', message: result.code === 'notConnected'
          ? 'O League of Legends desconectou. Aguarde a reconexão e confira os convites antes de tentar novamente.'
          : `Não foi possível concluir: ${result.message}` });
      }
    } catch {
      if (!mounted.current) return;
      setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e os convites antes de tentar novamente.' });
    } finally {
      inFlight.current.delete(key);
      if (mounted.current) {
        setPending(current => {
          const next = new Set(current);
          next.delete(key);
          return next;
        });
      }
    }
  }

  const invite = (friend: Friend) => execute(
    // A LCU só entrega o convite com o `summonerId`; o `puuid` vai junto, como no client oficial.
    { type: 'inviteToLobby', invitees: [{ summonerId: friend.summonerId, puuid: friend.puuid }] },
    `Convite enviado para ${friendDisplayName(friend)}.`,
  );
  const inviteRestriction = !lobby ? null
    : !lobby.canInvite ? 'Só o líder pode convidar neste lobby.'
    : !canInvite ? 'Os convites só podem ser enviados na fase de lobby, fora da busca.'
    : slots <= 0 ? 'O grupo está cheio.'
    : null;

  return (
    <Card as="section" aria-label="Convites" className="flex w-full flex-col gap-3 p-4 text-ink">
      <h2 className="text-section font-semibold">Convites</h2>

      <h3 className={subheading}>Recebidos</h3>
      {received.length === 0 ? <p className="text-body text-muted">Nenhum convite recebido.</p> : (
        <ul aria-label="Convites recebidos" className="flex flex-col gap-2">
          {received.map(invitation => (
            <ReceivedItem
              key={invitation.invitationId} invitation={invitation} busy={busy(`invitation:${invitation.invitationId}`)}
              onAccept={() => { void execute({ type: 'acceptInvitation', invitationId: invitation.invitationId },
                `Convite de ${inviterName(invitation)} aceito.`); }}
              onDecline={() => { void execute({ type: 'declineInvitation', invitationId: invitation.invitationId },
                `Convite de ${inviterName(invitation)} recusado.`); }}
            />
          ))}
        </ul>
      )}

      <h3 className={subheading}>Convidar amigos</h3>
      {!lobby ? <p className="text-body text-muted">Entre em um lobby para convidar amigos.</p> : (
        <>
          <p className="text-caption text-muted">
            {slots > 0 ? `${slots} ${slots === 1 ? 'vaga livre' : 'vagas livres'} no grupo.` : 'Nenhuma vaga livre no grupo.'}
          </p>
          {inviteRestriction && <p className="text-caption text-warn">{inviteRestriction}</p>}
          {candidates.length === 0 ? <p className="text-body text-muted">Nenhum amigo disponível para convidar.</p> : (
            <ul aria-label="Amigos para convidar" className="-mx-2 flex flex-col">
              {candidates.map(friend => {
                const name = friendDisplayName(friend);
                const key = `invite:${friend.puuid}`;
                return (
                  <li key={friend.id} className="flex items-center justify-between gap-3 rounded-control px-2 py-1 hover:bg-raised">
                    <span className="truncate text-body" title={name}>{name}</span>
                    <Button
                      size="sm" aria-label={`Convidar ${name}`} loading={pending.has(key)}
                      disabled={!canInvite || slots <= 0 || busy(key)}
                      onClick={() => { void invite(friend); }}
                    >Convidar</Button>
                  </li>
                );
              })}
            </ul>
          )}
          {lobby.invitations.length > 0 && (
            <>
              <h3 className={subheading}>Enviados pelo grupo</h3>
              <ul aria-label="Convites enviados" className="flex flex-col gap-1">
                {lobby.invitations.map((invitation, index) => (
                  <li key={`${invitation.puuid}:${index}`} className="flex justify-between gap-3 text-body">
                    <span className="truncate">{inviteeName(invitation.puuid, friends)}</span>
                    <span className="shrink-0 text-caption text-muted">{invitationStateNames[invitation.state]}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
      {feedback.message && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'}
          className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>
      )}
    </Card>
  );
}

function ReceivedItem({ invitation, busy, onAccept, onDecline }: {
  invitation: ReceivedInvitation; busy: boolean; onAccept: () => void; onDecline: () => void;
}) {
  const name = inviterName(invitation);
  return (
    <li className="flex flex-col gap-2 rounded-card border border-accent bg-accent-soft p-3">
      <span className="flex items-start gap-2">
        <Avatar name={name} size={32} />
        <span className="min-w-0 break-words text-body"><strong>{name}</strong> chamou você para {invitationQueueName(invitation)}.</span>
      </span>
      {!invitation.canAccept && <span className="text-caption text-warn">Não dá para aceitar agora (na fila ou em partida).</span>}
      <span className="flex gap-2">
        <Button variant="primary" size="sm" aria-label={`Aceitar convite de ${name}`} aria-busy={busy}
          disabled={busy || !invitation.canAccept} onClick={onAccept}>Aceitar</Button>
        <Button size="sm" aria-label={`Recusar convite de ${name}`} aria-busy={busy}
          disabled={busy} onClick={onDecline}>Recusar</Button>
      </span>
    </li>
  );
}
