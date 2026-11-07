import { useEffect, useId, useState, type ReactNode } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { availabilityColors, availabilityNames, describeActivity, friendNick, friendRiotId, groupFriends, isOnline } from '../agent/friends';
import type { Friend } from '../bindings/Friend';
import { Avatar, Card, SearchField } from './base';
import { cx } from './base/cx';

type Props = {
  connection: ConnectionSnapshot;
  /**
   * Ações de cada amigo (abrir chat na #31, convidar na #32). Ficam no fim da
   * linha do amigo; a lista não sabe o que elas fazem.
   */
  renderActions?: (friend: Friend) => ReactNode;
};

/** Intervalo de atualização do "há N min" da atividade dos amigos. */
const CLOCK_INTERVAL_MS = 30_000;

function useNow(intervalMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(timer);
  }, [intervalMs]);
  return now;
}

/**
 * Lista de amigos com presença (#30). Mostra os amigos online nas pastas do
 * client oficial, cada pasta recolhível, e os offline numa seção no fim, com o
 * que cada um está fazendo. Cada amigo aparece só pelo nick; o Riot ID com a
 * tag fica no tooltip.
 */
export function FriendsList({ connection, renderActions }: Props) {
  const state = connection.state;
  const connected = connection.status === 'connected' && state?.lcu.state === 'connected';
  const friends = connected ? state.friends : [];
  const groups = connected ? state.friendGroups : [];
  const [filter, setFilter] = useState('');
  /** Pastas que o jogador abriu ou fechou aqui; as outras seguem o client oficial. */
  const [toggled, setToggled] = useState<Record<string, boolean>>({});
  const filterId = useId();
  const now = useNow(CLOCK_INTERVAL_MS);

  if (!connected) {
    return (
      <Card as="section" aria-label="Amigos" className="flex w-full flex-col gap-2 p-4 text-ink">
        <h2 className="text-section font-semibold">Amigos</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends para ver os amigos.</p>
      </Card>
    );
  }

  const query = filter.trim().toLowerCase();
  const visible = query
    ? friends.filter(friend => `${friendRiotId(friend)} ${friend.name}`.toLowerCase().includes(query))
    : friends;
  const offline = visible.filter(friend => !isOnline(friend));
  const onlineTotal = friends.filter(isOnline).length;
  const totals = new Map(groupFriends(friends, groups).map(folder => [folder.id, folder.friends.length]));
  const folders = groupFriends(visible.filter(isOnline), groups);
  const item = (friend: Friend) => <FriendItem key={friend.id} friend={friend} now={now} actions={renderActions?.(friend)} />;
  const isOpen = (key: string, startsOpen: boolean) => !!query || (toggled[key] ?? startsOpen);
  const toggle = (key: string, startsOpen: boolean) =>
    setToggled(current => ({ ...current, [key]: !(current[key] ?? startsOpen) }));

  return (
    <Card as="section" aria-label="Amigos" className="flex w-full flex-col gap-3 p-4 text-ink">
      <h2 className="text-section font-semibold">
        Amigos <span className="text-caption font-normal text-muted">({onlineTotal} de {friends.length} online)</span>
      </h2>
      {friends.length === 0 ? <p className="text-body text-muted">Você ainda não tem amigos na lista.</p> : (
        <>
          <SearchField id={filterId} label="Buscar amigo" value={filter} placeholder="Buscar amigo"
            onChange={event => setFilter(event.target.value)} />
          {visible.length === 0 && <p className="text-body text-muted">Nenhum amigo encontrado.</p>}
          {!query && onlineTotal === 0 && <p className="text-body text-muted">Nenhum amigo online agora.</p>}
          {folders.map(folder => {
            const key = `pasta-${folder.id}`;
            return (
              <Folder key={key} id={key} title={folder.name} count={`${folder.friends.length}/${totals.get(folder.id) ?? folder.friends.length}`}
                open={isOpen(key, !folder.collapsed)} onToggle={() => toggle(key, !folder.collapsed)}>
                <ul aria-label={`Amigos online em ${folder.name}`} className="-mx-2 flex flex-col">{folder.friends.map(item)}</ul>
              </Folder>
            );
          })}
          {offline.length > 0 && (
            <Folder id="pasta-offline" title="Offline" count={String(offline.length)}
              open={isOpen('offline', false)} onToggle={() => toggle('offline', false)}>
              <ul aria-label="Amigos offline" className="-mx-2 flex flex-col">{offline.map(item)}</ul>
            </Folder>
          )}
        </>
      )}
    </Card>
  );
}

/** Pasta recolhível: o título abre e fecha a lista. */
function Folder({ id, title, count, open, onToggle, children }: {
  id: string; title: string; count: string; open: boolean; onToggle: () => void; children: ReactNode;
}) {
  const listId = useId();
  return (
    <div className="flex flex-col gap-1" data-folder={id}>
      <button type="button" aria-expanded={open} aria-controls={listId} onClick={onToggle}
        className="flex items-center gap-1 rounded-control-sm text-left text-caption font-semibold uppercase tracking-[0.06em] text-muted hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent">
        <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"
          strokeLinecap="round" strokeLinejoin="round" className={cx('shrink-0 transition-transform', open && 'rotate-90')}>
          <path d="M9 6l6 6-6 6" />
        </svg>
        <span className="min-w-0 truncate">{title}</span> <span className="shrink-0 font-normal">({count})</span>
      </button>
      <div id={listId} hidden={!open}>{children}</div>
    </div>
  );
}

function FriendItem({ friend, now, actions }: { friend: Friend; now: number; actions?: ReactNode }) {
  const name = friendNick(friend);
  const riotId = friendRiotId(friend);
  const activity = describeActivity(friend, now);
  const offline = !isOnline(friend);
  return (
    <li className={cx('flex items-center gap-3 rounded-control px-2 py-1.5 hover:bg-raised', offline && 'opacity-60')}>
      <span className="relative shrink-0">
        <Avatar name={name} size={36} src={friend.profileIconId !== null ? assetUrl('profileIcon', friend.profileIconId) : undefined} />
        <span
          role="img" aria-label={availabilityNames[friend.availability]} title={availabilityNames[friend.availability]}
          className={cx('absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full ring-2 ring-surface', availabilityColors[friend.availability])}
        />
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-body font-semibold" title={riotId}>{name}</span>
        <span className="truncate text-caption text-muted">{activity}</span>
        {friend.statusMessage && <span className="truncate text-caption text-muted italic" title={friend.statusMessage}>{friend.statusMessage}</span>}
      </span>
      {actions && <span className="flex shrink-0 gap-1">{actions}</span>}
    </li>
  );
}
