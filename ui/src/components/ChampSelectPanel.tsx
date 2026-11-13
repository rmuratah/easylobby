import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import {
  actingCells, champSelectPhaseLabels, championName, filterByLane, gridChampions, gridPurpose, hasLanes, isAramChampSelect, isBanPhase, markedChampion,
  lockedPick, myAction, playerName, remainingMs, searchChampions, type GridChampion, type MyAction,
} from '../agent/champSelect';
import { useCommand, type CommandFeedback } from '../agent/useCommand';
import { SpellIcon, SummonerSpellsPicker } from './SummonerSpellsPicker';
import { ChampSelectSwaps, SwapOffers } from './ChampSelectSwaps';
import { ChampSelectChatPanel } from './ChampSelectChatPanel';
import { RuneQuickPick } from './RuneQuickPick';
import { SkinCarousel } from './SkinCarousel';
import { AramChampSelectContent } from './AramChampSelect';
import { LANES, positionNames, type Lane } from '../agent/positions';
import { PositionIcon } from './PositionIcon';
import type { AgentState } from '../bindings/AgentState';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampSelectPlayer } from '../bindings/ChampSelectPlayer';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';
import { Button } from './base';
import { cx } from './base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send']; onOpenRunes?: () => void };

export function ChampionIcon({ id, size = 40, dim = false, gray = false }: { id: number | null; size?: number; dim?: boolean; gray?: boolean }) {
  // O fundo fica no lugar da imagem enquanto ela carrega ou quando não há ícone.
  return (
    <span aria-hidden="true" className={cx('inline-block shrink-0 overflow-hidden rounded-control-sm bg-raised', dim && 'opacity-50', gray && 'grayscale')}
      style={{ width: size, height: size }}>
      {id ? <img src={assetUrl('champion', id)} alt="" width={size} height={size}
        onError={(event) => { event.currentTarget.style.visibility = 'hidden'; }} /> : null}
    </span>
  );
}

/**
 * Tela da seleção de campeões (#28): os dois times, o timer da etapa, os
 * bans e a grade de campeões com busca. Mostra só o estado confirmado pelo
 * agente. Na vez do jogador, clicar na grade marca o campeão (hover) e o
 * botão confirma o ban ou o pick; antes da vez, marca a intenção (#23).
 */
export function ChampSelectPanel({ connection, send, onOpenRunes }: Props) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected') return null;
  if (!state.champSelect) {
    if (state.gameflowPhase !== 'ChampSelect') return null;
    return (
      <section aria-label="Seleção de campeões" className="rounded-card border border-edge bg-surface card p-5 text-ink">
        <p role="status" className="text-body text-muted">Carregando a seleção de campeões…</p>
      </section>
    );
  }
  // Cada seleção (e cada conexão) começa do zero: busca e escolha locais não passam para a próxima.
  // O ARAM tem tela própria (#201): campeão sorteado, reroll e banco, sem ban nem pick.
  const Content = isAramChampSelect(state.champSelect) ? AramChampSelectContent : ChampSelectContent;
  return <Content key={`${state.lcu.session}:${state.champSelect.id}`} state={state} champSelect={state.champSelect} send={send} onOpenRunes={onOpenRunes} />;
}

export type ContentProps = { state: AgentState; champSelect: ChampSelect; send: AgentClient['send']; onOpenRunes?: () => void };

function ChampSelectContent({ state, champSelect, send, onOpenRunes }: ContentProps) {
  const catalog = state.championCatalog;
  const acting = actingCells(champSelect);
  const mine = myAction(champSelect);
  // A escolha na grade vale só para a ação em que foi feita; depois vale o que a LCU marcou.
  const [choice, setChoice] = useState<{ actionId: number; championId: number } | null>(null);
  const chosen = mine && choice?.actionId === mine.action.id ? choice.championId : mine && markedChampion(champSelect, mine);
  const hover = useCommand(send);
  const confirm = useCommand(send);
  const swap = useCommand(send);
  // Os adversários usam a altura dos cards do seu time (#434).
  const [allyCardHeight, setAllyCardHeight] = useState<number | null>(null);
  function select(championId: number) {
    if (!mine || confirm.pending) return;
    setChoice({ actionId: mine.action.id, championId });
    void hover.run({ type: 'hoverChampion', actionId: mine.action.id, championId },
      { pending: '', failure: 'marcar o campeão' });
  }
  function lockIn(championId: number) {
    if (!mine?.turn || confirm.pending) return;
    const verb = mine.action.type === 'ban' ? 'banir' : 'escolher';
    void confirm.run({ type: 'confirmAction', actionId: mine.action.id, championId },
      { pending: mine.action.type === 'ban' ? 'Confirmando o ban…' : 'Confirmando a escolha…', failure: verb });
  }
  const me = champSelect.myTeam.find(player => player.isLocalPlayer);
  const lockedChampion = lockedPick(champSelect);
  const grid = gridChampions(state, champSelect);
  const chosenAvailable = !!chosen && grid.some(item => item.champion.id === chosen && item.available);
  // Na fase de banimento (#423) a tela ganha detalhes vermelhos (estilos em index.css).
  return (
    <section aria-label="Seleção de campeões" className="champ-select text-ink" data-ban-phase={isBanPhase(champSelect) || undefined}>
      <ChampSelectStatus champSelect={champSelect} />
      <AutofillNotice champSelect={champSelect} send={send} />
      <div className="champ-select-board">
        <div className="flex min-w-0 flex-col gap-4">
          {/* Os bans de cada time ficam em cima do nome dele (#475). */}
          <TeamBans label="Bans do seu time" ids={champSelect.bans.myTeam} slots={banSlots(champSelect, true)} catalog={catalog} />
          <TeamList label="Seu time" players={champSelect.myTeam} ally acting={acting} catalog={catalog}
            onCardHeight={setAllyCardHeight}
            renderActions={player => player.isLocalPlayer ? null
              : <SwapOffers champSelect={champSelect} cellId={player.cellId} busy={swap.pending} run={swap.run} />} />
          <SummonerSpellsPicker champSelect={champSelect} catalog={state.summonerSpellCatalog}
            summonerLevel={state.summoner?.summonerLevel ?? null} queues={state.queues} send={send} />
          <RuneQuickPick runePages={state.runePages} catalog={state.runeCatalog} send={send} onEdit={onOpenRunes} />
        </div>
        <div className="champ-select-center">
          <ChampSelectSwaps champSelect={champSelect} busy={swap.pending} run={swap.run} feedback={swap.feedback} />
          {/* Com o campeão confirmado, a seção de campeões vira a escolha de skin (#477). */}
          {me && lockedChampion ? (
            <SkinCarousel state={state} send={send} championId={lockedChampion}
              championName={championName(catalog, lockedChampion)} selectedSkinId={me.selectedSkinId} />
          ) : (
          // O botão de banir e escolher fica dentro da seção de campeões, embaixo e no meio (#475).
          <ChampionGrid state={state} champSelect={champSelect} grid={grid} selected={chosen || null}
            onSelect={mine && !confirm.pending ? select : null}
            footer={<ActionBar mine={mine} chosen={chosen || null} chosenAvailable={chosenAvailable} catalog={catalog}
              busy={confirm.pending} onConfirm={lockIn} feedback={[confirm.feedback, hover.feedback]} />} />
          )}
        </div>
        <div className="flex min-w-0 flex-col gap-4">
          <TeamBans label="Bans do time adversário" ids={champSelect.bans.theirTeam} slots={banSlots(champSelect, false)} catalog={catalog} />
          <TeamList label="Time adversário" players={champSelect.theirTeam} ally={false} acting={acting} catalog={catalog}
            minCardHeight={allyCardHeight} />
          {state.gameflowPhase === 'ChampSelect' && <ChampSelectChatPanel champSelect={champSelect} chat={state.champSelectChat} send={send} />}
        </div>
      </div>
    </section>
  );
}

export function Countdown({ champSelect }: { champSelect: ChampSelect }) {
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
  if (isBanPhase(champSelect)) return 'Fase de banimento: aguardando os outros jogadores.';
  if (champSelect.timer.phase === 'PLANNING') return 'Mostre ao time o campeão que pretende jogar.';
  if (champSelect.timer.phase === 'FINALIZATION') return 'Ajuste feitiços e runas antes da partida.';
  return 'Aguardando os outros jogadores.';
}

function ChampSelectStatus({ champSelect }: { champSelect: ChampSelect }) {
  const ban = isBanPhase(champSelect);
  return (
    <div className={cx('champ-select-status flex flex-wrap items-center gap-x-6 gap-y-2 rounded-card border bg-surface card px-5 py-3 text-ink',
      ban ? 'is-ban' : 'border-edge')}>
      <div className="flex min-w-0 flex-1 flex-col">
        <p className="text-caption text-muted">Etapa: <strong className="font-semibold text-ink">{ban ? 'Banimento' : champSelectPhaseLabels[champSelect.timer.phase]}</strong></p>
        <p role="status" className={cx('text-section font-semibold', ban ? 'text-danger' : 'text-accent-ink')}>{turnLabel(champSelect)}</p>
      </div>
      {/* A chave recria o relógio quando a etapa muda, sem esperar o próximo tique. */}
      <p className="flex items-baseline gap-2 text-caption text-muted">Tempo:
        <span className={cx('text-[2rem] font-semibold leading-none tabular-nums', ban ? 'text-danger' : 'text-ink')}>
          <Countdown key={`${champSelect.timer.phase}:${champSelect.timer.endsAtMs}`} champSelect={champSelect} />
        </span>
      </p>
      {/* Barra do tempo da etapa (#475): começa cheia e esvazia da direita para a esquerda. */}
      <TimeBar key={`${champSelect.timer.phase}:${champSelect.timer.endsAtMs}`} champSelect={champSelect} ban={ban} />
    </div>
  );
}

function TimeBar({ champSelect, ban }: { champSelect: ChampSelect; ban: boolean }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(interval);
  }, []);
  const total = champSelect.timer.totalTimeMs;
  if (champSelect.timer.isInfinite || total <= 0) return null;
  const left = Math.min(1, remainingMs(champSelect, now) / total);
  return (
    <div aria-hidden="true" className="h-1.5 w-full basis-full overflow-hidden rounded-full bg-raised">
      <div className={cx('h-full rounded-full transition-[width] duration-300 ease-linear', ban ? 'bg-danger' : 'bg-accent')}
        style={{ width: `${left * 100}%` }} />
    </div>
  );
}

/**
 * Aviso de autofill (#360), como o do client oficial: o jogador caiu numa
 * posição que não escolheu. "Entendi" marca como visto, e o agente esconde o
 * aviso até o fim desta seleção.
 */
function AutofillNotice({ champSelect, send }: { champSelect: ChampSelect; send: AgentClient['send'] }) {
  const ack = useCommand(send);
  const me = champSelect.myTeam.find(player => player.isLocalPlayer);
  if (!champSelect.autofillNotice || !me) return null;
  const position = me.assignedPosition !== 'unselected' && me.assignedPosition !== 'fill'
    ? positionNames[me.assignedPosition] : null;
  return (
    <section aria-label="Aviso de autofill"
      className="flex flex-wrap items-center gap-3 rounded-card border border-accent bg-accent-soft px-5 py-3 text-ink">
      {me.assignedPosition !== 'unselected' && <PositionIcon position={me.assignedPosition} size={24} />}
      <div className="min-w-0 flex-1">
        <p className="text-body font-semibold">
          {position ? `Você foi colocado em ${position} (autofill).` : 'Você foi colocado numa posição que não escolheu (autofill).'}
        </p>
        <p className="text-caption text-muted">O time precisava dessa posição. Na próxima partida, a fila tenta te dar uma das suas.</p>
        {ack.feedback.status === 'error' && <p role="alert" className="text-caption text-danger">{ack.feedback.message}</p>}
      </div>
      <Button size="sm" loading={ack.pending}
        onClick={() => void ack.run({ type: 'ackAutofill' }, { pending: '', failure: 'marcar o aviso como visto' })}>
        Entendi
      </Button>
    </section>
  );
}

type TeamProps = {
  label: string;
  players: ChampSelectPlayer[];
  ally: boolean;
  acting: Set<number>;
  catalog: ChampionCatalog | null;
  /** Botões extras de cada jogador (as trocas com um aliado, por exemplo). */
  renderActions?: (player: ChampSelectPlayer) => ReactNode;
  /** Avisa a altura dos cards desta lista (iguais entre si, #428) quando ela muda. */
  onCardHeight?: (height: number) => void;
  /** Altura mínima de cada card, para igualar à do outro time (#434). */
  minCardHeight?: number | null;
};

export function TeamList({ label, players, ally, acting, catalog, renderActions, onCardHeight, minCardHeight }: TeamProps) {
  const listRef = useRef<HTMLUListElement>(null);
  const firstCell = players[0]?.cellId;
  useEffect(() => {
    const card = listRef.current?.firstElementChild;
    if (!onCardHeight || !card || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => onCardHeight(card.getBoundingClientRect().height));
    observer.observe(card);
    return () => observer.disconnect();
  }, [onCardHeight, players.length, firstCell]);
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-2">
      <h3 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">{label}</h3>
      {players.length === 0 ? <p className="text-body text-muted">Sem jogadores.</p> : (
        // Todos os cards do time com a altura do maior (#428): as linhas da grade dividem o espaço por igual.
        // Uma coluna que não passa da largura do time: o nome longo é cortado em vez de esticar os cards (#475).
        <ul ref={listRef} className="grid grid-cols-[minmax(0,1fr)] auto-rows-fr gap-2"
          style={minCardHeight ? { gridAutoRows: `minmax(${minCardHeight}px, 1fr)` } : undefined}>
          {players.map((player, index) => {
            const intent = !player.championId && player.championPickIntent ? player.championPickIntent : null;
            const champion = player.championId ?? intent;
            return (
              <li key={player.cellId} aria-current={player.isLocalPlayer ? 'true' : undefined}
                className={cx('flex min-w-0 items-center gap-2 rounded-card border p-2 text-ink',
                  acting.has(player.cellId) ? 'border-accent bg-accent-soft' : player.isLocalPlayer ? 'border-control bg-surface' : 'border-line bg-surface')}>
                <ChampionIcon id={champion} dim={intent !== null} />
                {ally && (player.spell1Id || player.spell2Id) ? (
                  <span className="flex flex-col gap-0.5" aria-hidden="true">
                    <SpellIcon id={player.spell1Id} size={18} />
                    <SpellIcon id={player.spell2Id} size={18} />
                  </span>
                ) : null}
                <div className="min-w-0">
                  <p className="truncate text-body font-semibold">
                    {playerName(player, index, ally)}{player.isLocalPlayer && ' (você)'}
                  </p>
                  {/* No seu time o card mostra só o ícone do campeão (#475); o nome fica para o leitor de tela. */}
                  <p className="text-caption text-muted">
                    {ally ? <span className="sr-only">{champion ? championName(catalog, champion) : 'Sem campeão'}
                      {(player.assignedPosition !== 'unselected' || acting.has(player.cellId)) && ' · '}</span>
                      : champion ? championName(catalog, champion) : 'Sem campeão'}
                    {ally && player.assignedPosition !== 'unselected' && positionNames[player.assignedPosition]}
                    {ally && player.isAutofilled && (
                      <span title="Posição de autofill: o jogador não escolheu esta posição" className="ml-1 inline-flex items-center gap-0.5 align-middle font-semibold text-accent-ink">
                        <PositionIcon position="fill" size={12} />(autofill)
                      </span>
                    )}
                    {acting.has(player.cellId) && (ally && player.assignedPosition === 'unselected' ? 'na vez' : ' · na vez')}
                  </p>
                  {renderActions?.(player)}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** Quantos bans o time tem nesta seleção: as ações de ban dele ou, se forem menos, os bans já feitos. */
function banSlots(champSelect: ChampSelect, ally: boolean): number {
  const actions = champSelect.actions.filter(action => action.type === 'ban' && action.isAllyAction === ally).length;
  return Math.max(actions, (ally ? champSelect.bans.myTeam : champSelect.bans.theirTeam).length);
}

/** Bans de um time, em cima da lista dele; os que faltam aparecem como espaços vazios. */
function TeamBans({ label, ids, slots, catalog }: { label: string; ids: number[]; slots: number; catalog: ChampionCatalog | null }) {
  if (slots === 0) return null;
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-1">
      <h3 className="text-caption text-muted">{label}</h3>
      <ul className="flex flex-wrap gap-1">
        {Array.from({ length: slots }, (_, index) => {
          const id = ids[index];
          return id ? (
            <li key={index} title={championName(catalog, id)}>
              {/* Banido fica em cinza (#475). */}
              <ChampionIcon id={id} size={28} dim gray />
              <span className="sr-only">{championName(catalog, id)}</span>
            </li>
          ) : (
            <li key={index} aria-hidden="true" className="h-7 w-7 rounded-control-sm border border-dashed border-line" />
          );
        })}
      </ul>
    </section>
  );
}

type ActionBarProps = {
  mine: MyAction | null;
  chosen: number | null;
  chosenAvailable: boolean;
  catalog: ChampionCatalog | null;
  busy: boolean;
  onConfirm: (championId: number) => void;
  feedback: CommandFeedback[];
};

function ActionBar({ mine, chosen, chosenAvailable, catalog, busy, onConfirm, feedback }: ActionBarProps) {
  // O retorno mais recente que ainda diz algo: o da confirmação primeiro. Marcar o campeão
  // não mostra "marcando…" (#311), só o erro se a LCU recusar.
  const shown = feedback.find(f => f.status !== 'idle' && f.message);
  const message = shown && <p role={shown.status === 'error' ? 'alert' : 'status'}
    className={shown.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{shown.message}</p>;
  if (!mine) return message ?? null;
  if (!mine.turn) {
    return (
      <div className="flex flex-col items-center gap-1 text-center">
        <p className="text-body text-muted">
          {chosen ? `Intenção: ${championName(catalog, chosen)}. ` : ''}Ainda não é sua vez; o campeão marcado aparece para o time.
        </p>
        {message}
      </div>
    );
  }
  const ban = mine.action.type === 'ban';
  return (
    <div className="flex flex-col items-center gap-1 text-center">
      <div className="flex flex-col items-center gap-1">
        {/* O botão mostra só "Banir" (#311) ou "Confirmar" (#475); o nome do campeão fica para o leitor de tela. */}
        <Button variant={ban ? 'danger' : 'primary'} size="lg" className="min-w-48" disabled={!chosen || !chosenAvailable || busy}
          aria-label={chosen ? `${ban ? 'Banir' : 'Confirmar'} ${championName(catalog, chosen)}` : undefined}
          onClick={() => { if (chosen) onConfirm(chosen); }}>
          {ban ? 'Banir' : 'Confirmar'}
        </Button>
        {!chosen && <p className="text-body text-muted">{ban ? 'Escolha na grade quem banir.' : 'Escolha seu campeão na grade.'}</p>}
        {chosen && !chosenAvailable && <p className="text-body text-warn">Esse campeão não está mais disponível.</p>}
      </div>
      {message}
    </div>
  );
}

type GridProps = {
  state: AgentState;
  champSelect: ChampSelect;
  grid: GridChampion[];
  selected: number | null;
  /** `null` quando o jogador não tem ação para marcar agora. */
  onSelect: ((id: number) => void) | null;
  /** Fica embaixo da grade, no meio: o botão de banir e escolher. */
  footer?: ReactNode;
};

function ChampionGrid({ state, champSelect, grid: all, selected, onSelect, footer }: GridProps) {
  const [query, setQuery] = useState('');
  const [lane, setLane] = useState<Lane | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const hintId = useId();
  const purpose = gridPurpose(champSelect);
  const byId = new Map(all.map(item => [item.champion.id, item]));
  const showLanes = hasLanes(state.championCatalog);
  const activeLane = showLanes ? lane : null;
  const shown = filterByLane(searchChampions(all.map(item => item.champion), query), activeLane).map(c => byId.get(c.id)!);
  const label = purpose === 'ban' ? 'Campeões para banir' : 'Campeões';
  // Digitar em qualquer lugar da seleção já busca (#130), sem roubar o que vai para um campo ou botão.
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key.length !== 1 || event.ctrlKey || event.metaKey || event.altKey || event.defaultPrevented) return;
      if (!/\p{L}/u.test(event.key)) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
      // Montada escondida (na página de runas, por exemplo), a seleção não pega o teclado.
      if (!searchRef.current || searchRef.current.closest('[hidden]')) return;
      event.preventDefault();
      setQuery(current => current + event.key);
      searchRef.current?.focus();
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);
  const first = shown.find(item => item.available);
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-3 rounded-card border border-edge bg-surface card p-4 text-ink">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="text-section font-semibold">{label}</h3>
        <input ref={searchRef} type="search" aria-label="Buscar campeão" placeholder="Digite para buscar" value={query}
          aria-keyshortcuts="Enter Escape" aria-describedby={hintId}
          onChange={event => setQuery(event.target.value)}
          onKeyDown={event => {
            if (event.key === 'Escape') { setQuery(''); return; }
            if (event.key === 'Enter' && query && first && onSelect) { event.preventDefault(); onSelect(first.champion.id); }
          }}
          className="h-9 min-w-32 flex-1 rounded-control border border-control bg-surface px-3 text-body text-ink placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent" />
        <span id={hintId} className="sr-only">Enter marca o primeiro campeão da busca; Esc limpa.</span>
        {showLanes && (
          <ul aria-label="Filtrar por rota" className="flex gap-1">
            {LANES.map(option => (
              <li key={option}>
                <button type="button" aria-pressed={activeLane === option} title={positionNames[option]}
                  aria-label={positionNames[option]}
                  // Clicar na rota ativa tira o filtro (#250).
                  onClick={() => setLane(current => current === option ? null : option)}
                  className="flex h-9 w-9 items-center justify-center rounded-control border border-control text-muted hover:bg-raised hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent aria-pressed:border-accent aria-pressed:bg-accent-soft aria-pressed:text-accent-ink">
                  <PositionIcon position={option} size={20} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {!state.championCatalog ? <p className="champion-grid-area text-body text-muted">Carregando campeões…</p>
        : shown.length === 0 ? <p className="champion-grid-area text-body text-muted">{query || activeLane ? 'Nenhum campeão encontrado.' : 'Nenhum campeão disponível.'}</p> : (
          <ul className="champion-grid champion-grid-area" aria-label="Grade de campeões">
            {shown.map(({ champion, available }) => (
              <li key={champion.id}>
                <button type="button" aria-pressed={selected === champion.id} disabled={!available || !onSelect}
                  title={available ? champion.name : `${champion.name} (indisponível)`}
                  onClick={() => onSelect?.(champion.id)}
                  className="flex w-full flex-col items-center gap-1 rounded-control border-2 border-transparent p-1 text-caption text-ink hover:bg-raised focus-visible:outline-2 focus-visible:outline-accent aria-pressed:border-accent aria-pressed:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent">
                  {/* Banido ou já escolhido fica em cinza (#475). */}
                  <ChampionIcon id={champion.id} size={48} gray={!available} />
                  <span className="w-full truncate text-center">{champion.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      {footer && <div className="flex min-h-[5.25rem] items-start justify-center border-t border-line pt-3">{footer}</div>}
    </section>
  );
}
