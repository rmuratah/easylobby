import { useEffect, useId, useState, type ReactNode } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { availabilityColors, availabilityNames, describeActivity, friendDisplayName, isOnline } from '../agent/friends';
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
 * Lista de amigos com presença (#30). Mostra os amigos na ordem do agente,
 * separados em online e offline, com o que cada um está fazendo.
 */
export function FriendsList({ connection, renderActions }: Props) {
  const state = connection.state;
  const connected = connection.status === 'connected' && state?.lcu.state === 'connected';
  const friends = connected ? state.friends : [];
  const [filter, setFilter] = useState('');
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
    ? friends.filter(friend => `${friendDisplayName(friend)} ${friend.name}`.toLowerCase().includes(query))
    : friends;
  const online = visible.filter(isOnline);
  const offline = visible.filter(friend => !isOnline(friend));
  const onlineTotal = friends.filter(isOnline).length;
  const item = (friend: Friend) => <FriendItem key={friend.id} friend={friend} now={now} actions={renderActions?.(friend)} />;

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
          {online.length > 0 && (
            <div className="flex flex-col gap-1">
              <p aria-hidden="true" className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Online ({online.length})</p>
              <ul aria-label="Amigos online" className="-mx-2 flex flex-col">{online.map(item)}</ul>
            </div>
          )}
          {offline.length > 0 && (
            <details open={!!query} className="group">
              <summary className="cursor-pointer rounded-control-sm text-caption font-semibold uppercase tracking-[0.06em] text-muted hover:text-ink">
                Offline ({offline.length})
              </summary>
              <ul aria-label="Amigos offline" className="-mx-2 mt-1 flex flex-col">{offline.map(item)}</ul>
            </details>
          )}
        </>
      )}
    </Card>
  );
}

function FriendItem({ friend, now, actions }: { friend: Friend; now: number; actions?: ReactNode }) {
  const name = friendDisplayName(friend);
  const activity = describeActivity(friend, now);
  const league = friend.activity?.type === 'league' ? friend.activity : null;
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
        <span className="truncate text-body font-semibold" title={name}>{name}</span>
        <span className="truncate text-caption text-muted">{activity}</span>
        {friend.statusMessage && <span className="truncate text-caption text-muted italic" title={friend.statusMessage}>{friend.statusMessage}</span>}
      </span>
      {league?.championId != null && league.status === 'inGame' && (
        <Icon src={assetUrl('champion', league.championId)} className="h-7 w-7 shrink-0 rounded-control-sm" label="Campeão da partida" />
      )}
      {actions && <span className="flex shrink-0 gap-1">{actions}</span>}
    </li>
  );
}

/** Imagem do agente que vira um quadrado vazio quando não carrega. */
function Icon({ src, className, label }: { src: string | null; className: string; label?: string }) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!src || failed === src) return <span aria-hidden="true" className={`block bg-raised ${className}`} />;
  return <img src={src} alt={label ?? ''} className={className} onError={() => setFailed(src)} />;
}
