import { useEffect, useId, useRef } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { friendDisplayName, playerName } from '../agent/friends';
import { isOpenInvitation } from '../agent/invitations';
import type { LobbyInvite } from '../agent/lobbyInvite';
import type { InvitationState } from '../bindings/InvitationState';
import { Avatar, Button } from './base';
import { cx } from './base/cx';

/** Lista de amigos para convidar, aberta pelo "+" da vaga livre (#206). */
export function InvitePicker({ invites, onClose }: { invites: LobbyInvite; onClose: () => void }) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    function onPointer(event: PointerEvent) { if (ref.current && !ref.current.contains(event.target as Node)) onClose(); }
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  }, [onClose]);
  return (
    <div ref={ref} role="dialog" aria-labelledby={titleId} tabIndex={-1}
      className="absolute inset-x-0 top-full z-20 mt-2 flex max-h-72 flex-col gap-2 overflow-y-auto rounded-panel bg-surface p-3 text-ink shadow-lift animate-fade-up focus-visible:outline-none">
      <h3 id={titleId} className="px-1 text-section font-semibold">Convidar amigos</h3>
      {invites.candidates.length === 0
        ? <p className="px-1 text-body text-muted">Nenhum amigo disponível para convidar agora.</p>
        : (
          <ul aria-label="Amigos disponíveis" className="flex flex-col gap-1">
            {invites.candidates.map(friend => {
              const name = friendDisplayName(friend);
              return (
                <li key={friend.puuid} className="flex items-center gap-3 rounded-control px-2 py-1.5 hover:bg-raised">
                  <Avatar name={friend.gameName || name} size={32}
                    src={friend.profileIconId !== null ? assetUrl('profileIcon', friend.profileIconId) : undefined} />
                  <span className="min-w-0 flex-1 truncate text-body font-semibold">{name}</span>
                  <Button size="sm" aria-label={`Convidar ${name}`} loading={invites.pending(friend.puuid)}
                    disabled={invites.pending(friend.puuid)} onClick={() => { void invites.invite(friend); }}>Convidar</Button>
                </li>
              );
            })}
          </ul>
        )}
    </div>
  );
}

type GroupEntry = { puuid: string; name: string; status: 'member' | InvitationState };

const statusText: Record<GroupEntry['status'], string> = {
  member: 'No grupo',
  Requested: 'Convite pendente',
  Pending: 'Convite pendente',
  OnHold: 'Convite em espera',
  Accepted: 'No grupo',
  Declined: 'Recusou o convite',
  Kicked: 'Saiu do grupo',
  Error: 'Convite não entregue',
  Unknown: 'Convite enviado',
};

function StatusMark({ status }: { status: GroupEntry['status'] }) {
  if (status === 'member' || status === 'Accepted') {
    return (
      <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
        strokeLinecap="round" strokeLinejoin="round" className="text-accent-ink"><path d="M5 12l5 5 9-10" /></svg>
    );
  }
  if (isOpenInvitation(status) || status === 'Unknown') return <span aria-hidden="true" className="w-4 text-center font-semibold leading-none text-muted">…</span>;
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
      strokeLinecap="round" className="text-danger"><path d="M6 6l12 12M18 6L6 18" /></svg>
  );
}

/**
 * Card "Grupo" ao lado do lobby (#206), como a aba do client oficial: quem
 * entrou (✓) e os convites mandados com o status ("…" pendente, ✕ recusado).
 */
export function LobbyGroupCard({ connection }: { connection: ConnectionSnapshot }) {
  const state = connection.state;
  const lobby = connection.status === 'connected' && state?.lcu.state === 'connected' ? state.lobby : null;
  if (!state || !lobby) return null;
  const me = state.summoner?.puuid;
  const entries: GroupEntry[] = [
    ...lobby.members.filter(m => m.puuid !== me)
      .map(m => ({ puuid: m.puuid, name: playerName(m.puuid, m.summonerId, state.friends), status: 'member' as const })),
    ...lobby.invitations.filter(i => !lobby.members.some(m => m.puuid === i.puuid))
      .map(i => ({ puuid: i.puuid, name: playerName(i.puuid, i.summonerId, state.friends), status: i.state })),
  ];
  return (
    <section aria-label="Grupo" className="flex w-full flex-col gap-2 rounded-panel surface p-4 text-ink shadow-soft animate-fade-up">
      <h2 className="px-1 text-section font-semibold">Grupo ({entries.length})</h2>
      {entries.length === 0
        ? <p className="px-1 text-body text-muted">Ninguém convidado ainda. Use o “+” de uma vaga livre ou arraste um amigo para o lobby.</p>
        : (
          <ul aria-label="Convidados" className="flex flex-col gap-1">
            {entries.map(entry => (
              <li key={entry.puuid} className="flex items-center gap-3 px-1 py-1">
                <span className="flex w-4 shrink-0 justify-center"><StatusMark status={entry.status} /></span>
                <span className={cx('min-w-0 flex-1 truncate text-body', entry.status === 'member' ? 'text-ink' : 'text-muted')}>{entry.name}</span>
                <span className="shrink-0 text-caption text-muted">{statusText[entry.status]}</span>
              </li>
            ))}
          </ul>
        )}
    </section>
  );
}
