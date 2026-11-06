import { useEffect, useId, useState, type ReactNode } from 'react';
import type { ConnectionSnapshot } from '../agent/client';
import { assetUrl } from '../agent/assets';
import { availabilityColors, availabilityNames, describeActivity, friendDisplayName, isOnline } from '../agent/friends';
import type { Friend } from '../bindings/Friend';

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
      <section aria-label="Amigos" className="w-full max-w-lg rounded border border-slate-700 p-4">
        <h2 className="text-xl font-semibold">Amigos</h2>
        <p className="text-slate-300">Aguarde a conexão com o agente e o League of Legends para ver os amigos.</p>
      </section>
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
    <section aria-label="Amigos" className="flex w-full max-w-lg flex-col gap-3 rounded border border-slate-700 p-4">
      <h2 className="text-xl font-semibold">
        Amigos <span className="text-base font-normal text-slate-300">({onlineTotal} de {friends.length} online)</span>
      </h2>
      {friends.length === 0 ? <p className="text-slate-300">Você ainda não tem amigos na lista.</p> : (
        <>
          <label htmlFor={filterId} className="sr-only">Buscar amigo</label>
          <input
            id={filterId} type="search" value={filter} placeholder="Buscar amigo"
            onChange={event => setFilter(event.target.value)}
            className="rounded border border-slate-600 bg-slate-900 p-2"
          />
          {visible.length === 0 && <p className="text-slate-300">Nenhum amigo encontrado.</p>}
          {online.length > 0 && (
            <ul aria-label="Amigos online" className="flex flex-col gap-1">{online.map(item)}</ul>
          )}
          {offline.length > 0 && (
            <details open={!!query}>
              <summary className="cursor-pointer text-slate-300">Offline ({offline.length})</summary>
              <ul aria-label="Amigos offline" className="mt-1 flex flex-col gap-1">{offline.map(item)}</ul>
            </details>
          )}
        </>
      )}
    </section>
  );
}

function FriendItem({ friend, now, actions }: { friend: Friend; now: number; actions?: ReactNode }) {
  const name = friendDisplayName(friend);
  const activity = describeActivity(friend, now);
  const league = friend.activity?.type === 'league' ? friend.activity : null;
  const offline = !isOnline(friend);
  return (
    <li className={`flex items-center gap-3 rounded px-2 py-1 ${offline ? 'opacity-60' : ''}`}>
      <span className="relative shrink-0">
        <Icon src={friend.profileIconId !== null ? assetUrl('profileIcon', friend.profileIconId) : null} className="h-9 w-9 rounded-full" />
        <span
          role="img" aria-label={availabilityNames[friend.availability]} title={availabilityNames[friend.availability]}
          className={`absolute -right-0.5 -bottom-0.5 h-3 w-3 rounded-full border-2 border-slate-950 ${availabilityColors[friend.availability]}`}
        />
      </span>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="truncate font-medium" title={name}>{name}</span>
        <span className="truncate text-sm text-slate-300">{activity}</span>
        {friend.statusMessage && <span className="truncate text-xs text-slate-400 italic" title={friend.statusMessage}>{friend.statusMessage}</span>}
      </span>
      {league?.championId != null && league.status === 'inGame' && (
        <Icon src={assetUrl('champion', league.championId)} className="h-7 w-7 shrink-0 rounded" label="Campeão da partida" />
      )}
      {actions && <span className="flex shrink-0 gap-1">{actions}</span>}
    </li>
  );
}

/** Imagem do agente que vira um quadrado vazio quando não carrega. */
function Icon({ src, className, label }: { src: string | null; className: string; label?: string }) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!src || failed === src) return <span aria-hidden="true" className={`block bg-slate-800 ${className}`} />;
  return <img src={src} alt={label ?? ''} className={className} onError={() => setFailed(src)} />;
}
