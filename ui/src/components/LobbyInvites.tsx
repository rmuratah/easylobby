import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { friendDisplayName, groupFriends, memberName } from '../agent/friends';
import { isOpenInvitation } from '../agent/invitations';
import { friendTarget, type InviteTarget, type LobbyInvite } from '../agent/lobbyInvite';
import type { Friend } from '../bindings/Friend';
import type { FriendGroup } from '../bindings/FriendGroup';
import type { InvitationState } from '../bindings/InvitationState';
import type { RecentPlayer } from '../bindings/RecentPlayer';
import { Avatar, Button } from './base';
import { cx } from './base/cx';

/** Se o amigo bate com o filtro de nome de jogo e tagline (sem diferenciar maiúsculas nem acentos). */
function matchesFilter(friend: Friend, name: string, tag: string): boolean {
  const norm = (text: string) => text.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase().trim();
  const wantedName = norm(name);
  const wantedTag = norm(tag.replace(/^#/, ''));
  const gameName = norm(friend.gameName || friend.name);
  return (!wantedName || gameName.includes(wantedName)) && (!wantedTag || norm(friend.tagLine).includes(wantedTag));
}

/** Chaves das seções que não são pastas de amigos. */
const BY_NAME = 'por-nome';
const RECENT = 'recentes';

/** Base para filtrar um jogador recente com o mesmo filtro dos amigos. */
const emptyName = { name: '' } as Friend;

function recentName(player: RecentPlayer): string {
  const name = player.gameName || player.summonerName || 'Jogador';
  return player.tagLine ? `${name}#${player.tagLine}` : name;
}

function ChevronIcon({ open }: { open: boolean }) {
  return (
    <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="currentColor"
      className={cx('shrink-0 transition-transform', open && 'rotate-90')}><path d="M8 5l9 7-9 7z" /></svg>
  );
}

function ClearButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button type="button" aria-label={label} onClick={onClick}
      className="flex size-6 shrink-0 items-center justify-center rounded-full text-muted hover:bg-raised hover:text-ink">
      <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
        strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
    </button>
  );
}

/**
 * Pop-up de convidar, aberto pelo "+" da vaga livre (#206), como no client
 * oficial: filtro por nome de jogo e tagline, os amigos disponíveis nas
 * pastas da lista de amigos (recolhíveis), marcar vários e "Enviar convites".
 */
export function InvitePicker({ invites, groups, recent, loadRecent, onClose }: {
  invites: LobbyInvite;
  groups: FriendGroup[];
  /** Jogadores das partidas recentes, para a seção "Recentes". */
  recent: RecentPlayer[];
  /** Pede ao agente os recentes de novo; chamado ao abrir. */
  loadRecent: () => void;
  onClose: () => void;
}) {
  const titleId = useId();
  const nameId = useId();
  const tagId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState('');
  const [tag, setTag] = useState('');
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [toggled, setToggled] = useState<ReadonlySet<number | string>>(new Set());
  const [byNameSending, setByNameSending] = useState(false);
  const [byNameSent, setByNameSent] = useState('');
  const [sending, setSending] = useState(false);
  const mounted = useRef(true);

  useEffect(() => { loadRecent(); }, [loadRecent]);

  useEffect(() => {
    mounted.current = true;
    nameRef.current?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') onClose(); }
    document.addEventListener('keydown', onKey);
    return () => {
      mounted.current = false;
      document.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  const filtering = name.trim() !== '' || tag.trim() !== '';
  const folders = groupFriends(invites.candidates.filter(f => matchesFilter(f, name, tag)), groups);
  const recentPlayers = recent.filter(p => !invites.taken(p.puuid) && matchesFilter(
    { ...emptyName, gameName: p.gameName || p.summonerName, tagLine: p.tagLine }, name, tag));
  // Quem saiu da lista (convidado, ficou offline) sai da seleção; o mesmo jogador pode estar nas duas listas.
  const targets = new Map<string, InviteTarget>();
  for (const friend of invites.candidates) if (selected.has(friend.puuid)) targets.set(friend.puuid, friendTarget(friend));
  for (const player of recent) {
    if (selected.has(player.puuid) && !targets.has(player.puuid) && !invites.taken(player.puuid)) {
      targets.set(player.puuid, { puuid: player.puuid, summonerId: player.summonerId, name: recentName(player) });
    }
  }
  const chosen = [...targets.values()];
  const full = chosen.length >= invites.slots;
  const riotId = name.trim() && tag.trim().replace(/^#/, '') ? `${name.trim()}#${tag.trim().replace(/^#/, '')}` : '';
  const isOpen = (key: number | string, collapsedByDefault: boolean) => filtering || collapsedByDefault === toggled.has(key);

  function toggleFriend(puuid: string) {
    setSelected(current => {
      const next = new Set(current);
      if (!next.delete(puuid)) next.add(puuid);
      return next;
    });
  }

  function toggleFolder(id: number | string) {
    setToggled(current => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }

  async function submit() {
    if (chosen.length === 0 || sending) return;
    setSending(true);
    const ok = await invites.inviteMany(chosen);
    if (!mounted.current) return;
    setSending(false);
    if (ok) onClose();
  }

  async function inviteByName() {
    if (!riotId || byNameSending) return;
    setByNameSending(true);
    const ok = await invites.inviteByRiotId(name, tag);
    if (!mounted.current) return;
    setByNameSending(false);
    if (ok) setByNameSent(riotId);
  }

  function playerRow(key: string, puuid: string, title: string, subtitle: ReactNode, iconName: string, iconSrc: string | undefined, label: string) {
    const checked = selected.has(puuid);
    const busy = invites.pending(puuid);
    return (
      <li key={key}>
        <label className={cx('flex items-center gap-3 rounded-control px-2 py-1.5',
          (checked || !full) && !busy ? 'cursor-pointer hover:bg-raised' : 'opacity-60')}>
          <input type="checkbox" checked={checked} disabled={busy || (!checked && full)}
            onChange={() => toggleFriend(puuid)} aria-label={`Convidar ${label}`}
            className="size-4 shrink-0 accent-accent" />
          <Avatar name={iconName} size={32} src={iconSrc} />
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-body font-semibold">{title}</span>
            {subtitle && <span className="truncate text-caption text-muted">{subtitle}</span>}
          </span>
        </label>
      </li>
    );
  }

  function sectionHeader(key: number | string, label: string, open: boolean, count?: number) {
    return (
      <button type="button" aria-expanded={open} onClick={() => toggleFolder(key)} disabled={filtering}
        className="flex h-8 items-center gap-2 rounded-control-sm bg-raised px-3 text-left text-caption font-semibold uppercase tracking-wide text-muted hover:text-ink disabled:cursor-default">
        <ChevronIcon open={open} />
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {count !== undefined && <span className="shrink-0 font-normal normal-case">{count}</span>}
      </button>
    );
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex max-h-[min(40rem,calc(100vh-2rem))] w-full max-w-md flex-col gap-3 rounded-panel surface p-5 text-ink shadow-lift">
        <div className="flex items-center justify-between gap-2">
          <h2 id={titleId} className="px-1 text-section font-semibold">Convidar amigos</h2>
          <ClearButton label="Fechar" onClick={onClose} />
        </div>
        <fieldset className="flex flex-col gap-1.5">
          <legend className="px-1 pb-1.5 text-caption font-semibold text-muted">Digite um nome para filtrar</legend>
          <div className="flex h-10 items-center gap-2 rounded-control border border-control bg-surface px-3 focus-within:outline-2 focus-within:outline-offset-2 focus-within:outline-accent">
            <label htmlFor={nameId} className="sr-only">Nome de jogo</label>
            <input id={nameId} ref={nameRef} value={name} onChange={event => setName(event.target.value)} placeholder="Nome de jogo"
              autoComplete="off" className="min-w-0 flex-[3] input-bare bg-transparent text-body outline-none placeholder:text-muted" />
            {name && <ClearButton label="Limpar nome de jogo" onClick={() => setName('')} />}
            <span aria-hidden="true" className="text-muted">#</span>
            <label htmlFor={tagId} className="sr-only">Tagline</label>
            <input id={tagId} value={tag} onChange={event => setTag(event.target.value)} placeholder="Tagline"
              autoComplete="off" className="min-w-0 flex-[2] input-bare bg-transparent text-body outline-none placeholder:text-muted" />
            {tag && <ClearButton label="Limpar tagline" onClick={() => setTag('')} />}
          </div>
        </fieldset>
        <h3 className="px-1 text-caption font-semibold uppercase tracking-wide text-muted">Jogadores disponíveis</h3>
        <div className="-mx-1 flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto px-1">
          {/* Convidar por nome: qualquer jogador pelo Riot ID digitado no filtro. */}
          {(() => {
            const open = isOpen(BY_NAME, true) || !!riotId;
            return (
              <section aria-label="Convidar por nome" className="flex flex-col gap-1">
                {sectionHeader(BY_NAME, 'Convidar por nome', open)}
                {open && (riotId ? (
                  <div className="flex items-center gap-3 px-2 py-1.5">
                    <span className="min-w-0 flex-1 truncate text-body font-semibold">{riotId}</span>
                    {byNameSent === riotId
                      ? <span className="shrink-0 text-caption text-muted">Convite enviado</span>
                      : <Button size="sm" loading={byNameSending} onClick={() => { void inviteByName(); }}
                          disabled={invites.slots === 0} aria-label={`Convidar ${riotId} pelo nome`}>Convidar</Button>}
                  </div>
                ) : <p className="px-2 py-1 text-caption text-muted">Digite o nome de jogo e a tagline acima para convidar qualquer jogador.</p>)}
              </section>
            );
          })()}
          {(recentPlayers.length > 0 || !filtering) && (() => {
            const open = isOpen(RECENT, true);
            return (
              <section aria-label="Recentes" className="flex flex-col gap-1">
                {sectionHeader(RECENT, 'Recentes', open, recentPlayers.length)}
                {open && (recentPlayers.length === 0
                  ? <p className="px-2 py-1 text-caption text-muted">Nenhum jogador recente para convidar.</p>
                  : (
                    <ul className="flex flex-col">
                      {recentPlayers.map(player => {
                        const display = recentName(player);
                        return playerRow(`recente-${player.puuid}`, player.puuid, player.gameName || display, 'Jogou recentemente',
                          player.gameName || display, player.championId !== null ? assetUrl('champion', player.championId) : undefined, display);
                      })}
                    </ul>
                  ))}
              </section>
            );
          })()}
          {folders.length === 0 && (
            <p className="px-1 text-body text-muted">
              {filtering ? 'Nenhum amigo disponível com esse nome.' : 'Nenhum amigo disponível para convidar agora.'}
            </p>
          )}
          {folders.map(folder => {
            // Filtrando, as pastas com resultado abrem; sem filtro, vale o que o jogador deixou no client oficial.
            const open = isOpen(folder.id, folder.collapsed);
            return (
              <section key={folder.id} aria-label={folder.name} className="flex flex-col gap-1">
                {sectionHeader(folder.id, folder.name, open, folder.friends.length)}
                {open && (
                  <ul className="flex flex-col">
                    {folder.friends.map(friend => {
                      const display = friendDisplayName(friend);
                      return playerRow(friend.puuid, friend.puuid, friend.gameName || display, friend.tagLine ? display : null,
                        friend.gameName || display, friend.profileIconId !== null ? assetUrl('profileIcon', friend.profileIconId) : undefined, display);
                    })}
                  </ul>
                )}
              </section>
            );
          })}
        </div>
        {invites.error && <p role="alert" className="px-1 text-body text-danger">{invites.error}</p>}
        <div className="flex items-center justify-between gap-3 border-t border-line pt-3">
          <span className="text-caption text-muted">
            {full && chosen.length > 0 ? `Limite de vagas: ${invites.slots}` : `${chosen.length} selecionado${chosen.length === 1 ? '' : 's'}`}
          </span>
          <Button variant="primary" disabled={chosen.length === 0} loading={sending} onClick={() => { void submit(); }}>
            Enviar convites
          </Button>
        </div>
      </div>
    </div>,
    document.body,
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
  Kicked: 'Expulso do grupo',
  Left: 'Saiu do grupo',
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
      .map(m => ({ puuid: m.puuid, name: memberName(m, state.friends), status: 'member' as const })),
    ...lobby.invitations.filter(i => !lobby.members.some(m => m.puuid === i.puuid))
      .map(i => ({ puuid: i.puuid, name: memberName(i, state.friends), status: i.state })),
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
