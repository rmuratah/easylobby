import { useEffect, useState } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import {
  actingCells, champSelectPhaseLabels, championName, gridChampions, gridPurpose, playerName, remainingMs, searchChampions,
} from '../agent/champSelect';
import { positionNames } from '../agent/positions';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectPlayer } from '../bindings/ChampSelectPlayer';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

export function ChampionIcon({ id, size = 40, dim = false }: { id: number | null; size?: number; dim?: boolean }) {
  // O fundo fica no lugar da imagem enquanto ela carrega ou quando não há ícone.
  return (
    <span aria-hidden="true" className={`inline-block shrink-0 overflow-hidden rounded bg-slate-800 ${dim ? 'opacity-50' : ''}`}
      style={{ width: size, height: size }}>
      {id ? <img src={assetUrl('champion', id)} alt="" width={size} height={size}
        onError={(event) => { event.currentTarget.style.visibility = 'hidden'; }} /> : null}
    </span>
  );
}

/**
 * Tela da seleção de campeões (#28): os dois times, o timer da etapa, os
 * bans e a grade de campeões com busca. Mostra só o estado confirmado pelo
 * agente; a escolha na grade é local até as ações da seleção (#23).
 */
export function ChampSelectPanel({ connection, send }: Props) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected') return null;
  if (!state.champSelect) {
    if (state.gameflowPhase !== 'ChampSelect') return null;
    return (
      <section aria-label="Seleção de campeões" className="rounded border border-slate-700 p-4">
        <p role="status">Carregando a seleção de campeões…</p>
      </section>
    );
  }
  // Cada seleção (e cada conexão) começa do zero: busca e escolha locais não passam para a próxima.
  return <ChampSelectContent key={`${state.lcu.session}:${state.champSelect.id}`} state={state} champSelect={state.champSelect} send={send} />;
}

type ContentProps = { state: AgentState; champSelect: ChampSelect; send: AgentClient['send'] };

function ChampSelectContent({ state, champSelect }: ContentProps) {
  const [selected, setSelected] = useState<number | null>(null);
  const catalog = state.championCatalog;
  const acting = actingCells(champSelect);
  return (
    <section aria-label="Seleção de campeões" className="champ-select">
      <ChampSelectStatus champSelect={champSelect} />
      <div className="champ-select-board">
        <TeamList label="Seu time" players={champSelect.myTeam} ally acting={acting} catalog={catalog} />
        <div className="champ-select-center">
          <Bans champSelect={champSelect} catalog={catalog} />
          <ChampionGrid state={state} champSelect={champSelect} selected={selected} onSelect={setSelected} />
        </div>
        <TeamList label="Time adversário" players={champSelect.theirTeam} ally={false} acting={acting} catalog={catalog} />
      </div>
    </section>
  );
}

function Countdown({ champSelect }: { champSelect: ChampSelect }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(interval);
  }, []);
  if (champSelect.timer.isInfinite) return <strong aria-label="Tempo da etapa">sem limite</strong>;
  return <strong aria-label="Tempo da etapa">{Math.ceil(remainingMs(champSelect, now) / 1000)} s</strong>;
}

function turnLabel(champSelect: ChampSelect): string {
  const action = champSelect.pendingAction;
  if (action?.type === 'ban') return 'Sua vez de banir.';
  if (action?.type === 'pick') return 'Sua vez de escolher.';
  if (champSelect.timer.phase === 'PLANNING') return 'Mostre ao time o campeão que pretende jogar.';
  if (champSelect.timer.phase === 'FINALIZATION') return 'Ajuste feitiços e runas antes da partida.';
  return 'Aguardando os outros jogadores.';
}

function ChampSelectStatus({ champSelect }: { champSelect: ChampSelect }) {
  return (
    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
      <p>Etapa: <strong>{champSelectPhaseLabels[champSelect.timer.phase]}</strong></p>
      {/* A chave recria o relógio quando a etapa muda, sem esperar o próximo tique. */}
      <p>Tempo: <Countdown key={`${champSelect.timer.phase}:${champSelect.timer.endsAtMs}`} champSelect={champSelect} /></p>
      <p role="status" className="text-sky-300">{turnLabel(champSelect)}</p>
    </div>
  );
}

type TeamProps = {
  label: string;
  players: ChampSelectPlayer[];
  ally: boolean;
  acting: Set<number>;
  catalog: ChampionCatalog | null;
};

function TeamList({ label, players, ally, acting, catalog }: TeamProps) {
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-2">
      <h3 className="text-lg font-semibold">{label}</h3>
      {players.length === 0 ? <p className="text-slate-400">Sem jogadores.</p> : (
        <ul className="flex flex-col gap-2">
          {players.map((player, index) => {
            const intent = !player.championId && player.championPickIntent ? player.championPickIntent : null;
            const champion = player.championId ?? intent;
            return (
              <li key={player.cellId} aria-current={player.isLocalPlayer ? 'true' : undefined}
                className={`flex items-center gap-2 rounded border p-2 ${acting.has(player.cellId) ? 'border-sky-400' : 'border-slate-700'}`}>
                <ChampionIcon id={champion} dim={intent !== null} />
                <div className="min-w-0">
                  <p className="truncate font-medium">
                    {playerName(player, index, ally)}{player.isLocalPlayer && ' (você)'}
                  </p>
                  <p className="text-sm text-slate-300">
                    {champion ? championName(catalog, champion) + (intent ? ' (intenção)' : '') : 'Sem campeão'}
                    {ally && player.assignedPosition !== 'unselected' && ` · ${positionNames[player.assignedPosition]}`}
                    {acting.has(player.cellId) && ' · na vez'}
                  </p>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function Bans({ champSelect, catalog }: { champSelect: ChampSelect; catalog: ChampionCatalog | null }) {
  const teams = [['Bans do seu time', champSelect.bans.myTeam], ['Bans do time adversário', champSelect.bans.theirTeam]] as const;
  if (teams.every(([, ids]) => ids.length === 0)) return null;
  return (
    <div className="flex flex-wrap gap-4">
      {teams.map(([label, ids]) => ids.length > 0 && (
        <section key={label} aria-label={label}>
          <h3 className="text-sm text-slate-300">{label}</h3>
          <ul className="flex flex-wrap gap-1">
            {ids.map(id => (
              <li key={id} title={championName(catalog, id)}>
                <ChampionIcon id={id} size={28} dim />
                <span className="sr-only">{championName(catalog, id)}</span>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

type GridProps = {
  state: AgentState;
  champSelect: ChampSelect;
  selected: number | null;
  onSelect: (id: number) => void;
};

function ChampionGrid({ state, champSelect, selected, onSelect }: GridProps) {
  const [query, setQuery] = useState('');
  const purpose = gridPurpose(champSelect);
  const all = gridChampions(state, champSelect);
  const byId = new Map(all.map(item => [item.champion.id, item]));
  const shown = searchChampions(all.map(item => item.champion), query).map(c => byId.get(c.id)!);
  const label = purpose === 'ban' ? 'Campeões para banir' : 'Campeões';
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <h3 className="text-lg font-semibold">{label}</h3>
        <input type="search" aria-label="Buscar campeão" placeholder="Buscar campeão" value={query}
          onChange={event => setQuery(event.target.value)}
          className="min-w-0 flex-1 rounded border border-slate-600 bg-slate-900 px-2 py-1" />
      </div>
      {!state.championCatalog ? <p>Carregando campeões…</p>
        : shown.length === 0 ? <p>{query ? 'Nenhum campeão encontrado.' : 'Nenhum campeão disponível.'}</p> : (
          <ul className="champion-grid" aria-label="Grade de campeões">
            {shown.map(({ champion, available }) => (
              <li key={champion.id}>
                <button type="button" aria-pressed={selected === champion.id} disabled={!available}
                  title={available ? champion.name : `${champion.name} (indisponível)`}
                  onClick={() => onSelect(champion.id)}
                  className="flex w-full flex-col items-center gap-1 rounded border border-transparent p-1 text-xs aria-pressed:border-sky-400 aria-pressed:bg-sky-950 disabled:cursor-not-allowed disabled:opacity-40">
                  <ChampionIcon id={champion.id} size={48} />
                  <span className="w-full truncate text-center">{champion.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
    </section>
  );
}
