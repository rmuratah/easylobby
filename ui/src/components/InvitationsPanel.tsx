import { useEffect, useRef, useState, type ReactNode } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { friendDisplayName, friendRiotId } from '../agent/friends';
import { freeSlots, invitableFriends, invitationQueueName, invitationSender, inviterName } from '../agent/invitations';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import type { Friend } from '../bindings/Friend';
import type { GameQueue } from '../bindings/GameQueue';
import type { ReceivedInvitation } from '../bindings/ReceivedInvitation';
import { Avatar, Button, Card } from './base';

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient['send'];
  /** Recebe o botão "Convidar" de cada amigo (ou nada), para a lista de amigos mostrar na linha dele. */
  children: (inviteAction: (friend: Friend) => ReactNode) => ReactNode;
};
type InvitationCommand = Extract<Command, { type: 'acceptInvitation' | 'declineInvitation' | 'inviteToLobby' }>;
type Feedback = { status: 'idle' | 'success' | 'error'; message: string };

/**
 * Convites de lobby (#32), sem seção própria: os convites recebidos aparecem
 * só enquanto existem, com aceitar e recusar, e o "Convidar" fica na linha de
 * cada amigo que dá para chamar ao lobby agora.
 */
export function InvitationsPanel({ connection, send, children }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') return children(() => null);
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <InvitationsActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send}>{children}</InvitationsActions>;
}

/** Chave do comando em andamento, para não repetir o mesmo clique. */
function commandKey(command: InvitationCommand): string {
  return command.type === 'inviteToLobby' ? `invite:${command.invitees[0]?.puuid}` : `invitation:${command.invitationId}`;
}

function InvitationsActions({ state, send, children }: {
  state: AgentState; send: AgentClient['send']; children: Props['children'];
}) {
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
  const inviteAction = (friend: Friend) => {
    const key = `invite:${friend.puuid}`;
    const offered = canInvite && slots > 0 && candidates.some(c => c.puuid === friend.puuid);
    if (!offered && !pending.has(key)) return null;
    const name = friendDisplayName(friend);
    return (
      <Button size="sm" aria-label={`Convidar ${name}`} loading={pending.has(key)} disabled={busy(key)}
        onClick={() => { void invite(friend); }}>Convidar</Button>
    );
  };

  return (
    <>
      {received.length > 0 && (
        <Card as="section" aria-label="Convites recebidos" className="flex w-full flex-col gap-2 p-4 text-ink">
          <ul aria-label="Convites recebidos" className="flex flex-col gap-2">
            {received.map(invitation => (
              <ReceivedItem
                key={invitation.invitationId} invitation={invitation} friends={friends} queues={state.queues} busy={busy(`invitation:${invitation.invitationId}`)}
                onAccept={() => { void execute({ type: 'acceptInvitation', invitationId: invitation.invitationId },
                  `Convite de ${inviterName(invitation, friends)} aceito.`); }}
                onDecline={() => { void execute({ type: 'declineInvitation', invitationId: invitation.invitationId },
                  `Convite de ${inviterName(invitation, friends)} recusado.`); }}
              />
            ))}
          </ul>
        </Card>
      )}
      {feedback.message && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'}
          className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>
      )}
      {children(inviteAction)}
    </>
  );
}

function ReceivedItem({ invitation, friends, queues, busy, onAccept, onDecline }: {
  invitation: ReceivedInvitation; friends: Friend[]; queues: GameQueue[]; busy: boolean; onAccept: () => void; onDecline: () => void;
}) {
  const sender = invitationSender(invitation, friends);
  const name = inviterName(invitation, friends);
  const icon = sender?.profileIconId != null ? assetUrl('profileIcon', sender.profileIconId) : undefined;
  return (
    <li className="flex flex-col gap-2 rounded-card border border-accent bg-accent-soft p-3">
      <span className="flex items-start gap-2">
        <Avatar name={name} size={32} src={icon} />
        <span className="min-w-0 break-words text-body">
          <strong title={sender ? friendRiotId(sender) : undefined}>{name}</strong> chamou você
          para <strong>{invitationQueueName(invitation, queues)}</strong>.
        </span>
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
