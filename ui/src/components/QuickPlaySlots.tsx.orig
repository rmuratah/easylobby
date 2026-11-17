import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient } from '../agent/client';
import { filterByLane, hasLanes, searchChampions } from '../agent/champSelect';
import { LANES, positionNames, type Lane } from '../agent/positions';
import {
  draftComplete, draftFromSlots, missingScarcePosition, SLOT_POSITIONS, sameDraft, slotChampions, slotSkins, slotsCommand,
  withChampion, withPosition, type SlotExtras, type SlotPair,
} from '../agent/quickPlay';
import { spellName, spellsForQueue, spellUnlocked, withSpell } from '../agent/summonerSpells';
import { assetUrl } from '../agent/assets';
import { useCommand } from '../agent/useCommand';
import type { AgentState } from '../bindings/AgentState';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { Position } from '../bindings/Position';
import { cx } from './base/cx';
import { Segmented } from './base/Segmented';
import { ChampionIcon } from './ChampSelectPanel';
import { PositionIcon } from './PositionIcon';
import { RuneIcon } from './RunePagesPanel';
import { SpellIcon } from './SummonerSpellsPicker';
import type { PlayerSlot } from '../bindings/PlayerSlot';
import type { RuneCatalog } from '../bindings/RuneCatalog';
import type { RuneStyle } from '../bindings/RuneStyle';
import type { SlotRunesInput } from '../bindings/SlotRunesInput';
import {
  allowedSubStyles, draftFromPage, draftProblem, draftToInput, findStyle, newDraft, primarySlots, statSlots, subSlots,
  withPrimary, withPrimaryPerk, withStatPerk, withSubPerk, withSubStyle, type RuneDraft,
} from './runePage';

type Props = {
  member: LobbyMember;
  /** O card é do jogador logado: os slots viram botões. */
  mine: boolean;
  /** Fora da fase de lobby, na fila ou com partida encontrada, os slots ficam só para ver. */
  canEdit: boolean;
  state: AgentState;
  send: AgentClient['send'];
  /** Onde mostrar os avisos e o retorno do agente, fora do card, para ele não crescer. */
  notes?: HTMLElement | null;
};

type Tab = 'position' | 'champion' | 'skin' | 'spells' | 'runes';
type Open = { index: 0 | 1; tab: Tab; spell?: 0 | 1 };
const slotLabels = ['Primeira posição', 'Segunda posição'] as const;
const slotNumerals = ['I', 'II'] as const;
const tabLabels: Record<Tab, string> = { position: 'Posição', champion: 'Campeão', skin: 'Skins', spells: 'Feitiços', runes: 'Runas' };
const TABS: Tab[] = ['position', 'champion', 'skin', 'spells', 'runes'];
const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/** Marca das posições prioritárias (as que faltam na fila), como o losango azul do client oficial. */
function ScarceMark({ className }: { className?: string }) {
  return <span aria-hidden="true" className={cx('size-2.5 rotate-45 rounded-[2px] bg-accent ring-2 ring-surface', className)} />;
}

/**
 * Posições e campeões do Jogo Dinâmico (#218) no card do jogador no lobby: dois slots, cada um com a posição, o
 * campeão, os feitiços, as runas e a skin. No card do jogador logado, cada parte abre o pop-up "Escolha seus
 * campeões" (#407) já no slot e na aba certos; nos outros cards é só para ver. A escolha vai para o agente quando
 * os dois slots têm posição e campeão.
 */
export function QuickPlaySlots({ member, mine, canEdit, state, send, notes }: Props) {
  const command = useCommand(send);
  const confirmed = draftFromSlots(member.playerSlots);
  const confirmedKey = JSON.stringify(confirmed);
  const [draft, setDraft] = useState<{ key: string; slots: SlotPair }>({ key: confirmedKey, slots: confirmed });
  // Um lobby novo do agente vale mais que a escolha local, menos durante o envio.
  if (draft.key !== confirmedKey && !command.pending) setDraft({ key: confirmedKey, slots: confirmed });
  const [open, setOpen] = useState<Open | null>(null);
  const editable = mine && canEdit;
  // Na fila ou fora da fase de lobby, o pop-up fecha.
  if (open && !editable) setOpen(null);
  const pair = draft.slots;
  const scarce = state.lobby?.scarcePositions ?? [];
  const catalog = state.championCatalog;
  const championName = (id: number) => catalog?.champions.find(c => c.id === id)?.name ?? (id ? `Campeão ${id}` : 'Escolher campeão');
  /** O slot salvo no agente quando é o mesmo da tela: só ele tem feitiços, runas e skin para mexer. */
  const savedSlot = (index: 0 | 1): PlayerSlot | null => {
    const saved = member.playerSlots[index];
    const slot = pair[index];
    return saved && slot.championId > 0 && saved.championId === slot.championId && saved.position === slot.position ? saved : null;
  };

  async function save(next: SlotPair, extras?: SlotExtras) {
    setDraft({ key: confirmedKey, slots: next });
    if (!draftComplete(next) || (!extras && sameDraft(next, confirmed))) return;
    await command.run(slotsCommand(next, extras), { pending: 'Salvando posições e campeões…', failure: 'salvar as posições e os campeões' });
    // O agente publica o lobby antes de responder: a tela volta ao que ele confirmou.
    setDraft(current => ({ ...current, key: '' }));
  }

  const warning = missingScarcePosition(pair, scarce);
  const incomplete = !draftComplete(pair);
  const messages = mine && (
    <>
      {warning && <p role="status" className="text-caption text-warn">Escolha ao menos uma posição prioritária (marcada com o losango).</p>}
      {incomplete && editable && <p className="text-caption text-muted">Escolha as duas posições e um campeão para cada uma.</p>}
      {command.feedback.status !== 'idle' && (
        <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
          className={cx('text-caption', command.feedback.status === 'error' ? 'text-danger' : 'text-muted')}>
          {command.feedback.message}
        </p>
      )}
    </>
  );
  return (
    <div className="relative flex shrink-0 flex-col items-end gap-1">
      <ul aria-label={mine ? 'Suas posições e campeões' : 'Posições e campeões'} className="flex gap-4">
        {pair.map((slot, i) => {
          const index = i as 0 | 1;
          const saved = savedSlot(index);
          const show = (tab: Tab, spell?: 0 | 1) => setOpen({ index, tab, spell });
          return (
            <li key={index} className="flex items-center gap-1">
              <SlotButton editable={editable} label={`${slotLabels[index]}: ${slot.championId ? championName(slot.championId) : 'sem campeão'}`}
                disabled={command.pending} onClick={() => show('champion')}>
                <span className="relative block">
                  <ChampionIcon id={slot.championId || null} size={64} />
                  {!slot.championId && <span aria-hidden="true" className="absolute inset-0 flex items-center justify-center text-title text-muted">?</span>}
                </span>
              </SlotButton>
              <span className="flex flex-col items-start gap-1">
                <SlotButton editable={editable} label={`${slotLabels[index]}: ${positionNames[slot.position]}`}
                  disabled={command.pending} onClick={() => show('position')}>
                  <span className={cx('relative flex size-8 items-center justify-center rounded-control-sm bg-raised',
                    slot.position === 'unselected' ? 'text-muted' : 'text-ink')}>
                    <PositionIcon position={slot.position} size={20} />
                    {scarce.includes(slot.position) && <ScarceMark className="absolute -right-1 -top-1" />}
                  </span>
                </SlotButton>
                {saved && (
                  <span className="flex items-center gap-0.5">
                    {([0, 1] as const).map(spell => (
                      <SlotButton key={spell} editable={editable} disabled={command.pending}
                        label={`${slotLabels[index]}, feitiço ${spell === 0 ? 'D' : 'F'}: ${spellName(state.summonerSpellCatalog, (spell === 0 ? saved.spell1Id : saved.spell2Id) || null)}`}
                        onClick={() => show('spells', spell)}>
                        <SpellIcon id={(spell === 0 ? saved.spell1Id : saved.spell2Id) || null} size={26} />
                      </SlotButton>
                    ))}
                    <SlotButton editable={editable} disabled={command.pending} label={`${slotLabels[index]}: runas`}
                      onClick={() => show('runes')}>
                      <RuneIcon kind="perk" id={saved.keystoneId} size={26} />
                    </SlotButton>
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>
      {notes ? messages && createPortal(messages, notes) : messages}
      {editable && open && createPortal(
        <SlotsDialog open={open} onOpen={setOpen} onClose={() => setOpen(null)} pair={pair} savedSlot={savedSlot}
          state={state} send={send} pending={command.pending} scarce={scarce} championName={championName}
          messages={messages} onSave={(next, extras) => { void save(next, extras); }} />,
        document.body,
      )}
    </div>
  );
}

function SlotButton({ editable, label, disabled, onClick, children }: {
  editable: boolean; label: string; disabled: boolean; onClick: () => void; children: ReactNode;
}) {
  if (!editable) return <span role="img" aria-label={label} title={label}>{children}</span>;
  return (
    <button type="button" aria-label={label} title={label} aria-haspopup="dialog" disabled={disabled}
      onClick={onClick}
      className={cx('rounded-control-sm transition-transform enabled:hover:-translate-y-0.5 disabled:cursor-wait', focusRing)}>
      {children}
    </button>
  );
}

/** Para onde ir depois de uma escolha: o que ainda falta, primeiro no mesmo slot e depois no outro. */
function nextStep(pair: SlotPair, index: 0 | 1): Open | null {
  for (const i of [index, (1 - index) as 0 | 1]) {
    if (pair[i].position === 'unselected') return { index: i, tab: 'position' };
    if (!pair[i].championId) return { index: i, tab: 'champion' };
  }
  return null;
}

/**
 * Pop-up "Escolha seus campeões" (#407): os dois slots à esquerda, com o botão de inverter entre eles (#405), e a
 * escolha do slot ativo à direita, em abas (posição, campeão, skins, feitiços e runas). Cada escolha é salva na hora,
 * como no card.
 */
function SlotsDialog({ open, onOpen, onClose, pair, savedSlot, state, send, pending, scarce, championName, messages, onSave }: {
  open: Open; onOpen: (open: Open) => void; onClose: () => void; pair: SlotPair; savedSlot: (index: 0 | 1) => PlayerSlot | null;
  state: AgentState; send: AgentClient['send']; pending: boolean; scarce: Position[]; championName: (id: number) => string;
  messages: ReactNode; onSave: (next: SlotPair, extras?: SlotExtras) => void;
}) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; });
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('[role="tabpanel"] input, [role="tabpanel"] button[aria-pressed="true"], [role="tabpanel"] button')?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') closeRef.current(); }
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
      opener?.focus?.();
    };
  }, []);
  const { index, tab } = open;
  const slot = pair[index];
  const saved = savedSlot(index);
  const needsSaved = tab === 'skin' || tab === 'spells' || tab === 'runes';
  const complete = draftComplete(pair);

  function chooseSlots(next: SlotPair, advance: boolean) {
    onSave(next);
    if (!advance) return;
    const step = nextStep(next, index);
    if (step) onOpen(step);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-overlay p-4 animate-fade-up"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={ref} role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex h-[min(42rem,calc(100vh-2rem))] w-full max-w-5xl flex-col overflow-hidden rounded-panel bg-surface card text-ink shadow-lift">
        <header className="flex items-center justify-between gap-3 border-b border-line px-5 py-3">
          <h2 id={titleId} className="text-section font-semibold">Escolha seus campeões</h2>
          <button type="button" aria-label="Fechar" title="Fechar" onClick={onClose}
            className={cx('flex size-8 shrink-0 items-center justify-center rounded-full text-muted hover:bg-raised hover:text-ink', focusRing)}>
            <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"
              strokeLinecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
          </button>
        </header>
        <div className="grid min-h-0 flex-1 grid-rows-[auto_minmax(0,1fr)] md:grid-cols-[19rem_minmax(0,1fr)] md:grid-rows-1">
          <aside aria-label="Seus slots" className="flex flex-col gap-2 border-b border-line bg-raised/40 p-4 md:border-b-0 md:border-r">
            {([0, 1] as const).map(i => (
              <div key={i} className="contents">
                <SlotCard index={i} slot={pair[i]} saved={savedSlot(i)} active={i === index} tab={i === index ? tab : null} openSpell={open.spell}
                  state={state} scarce={scarce} championName={championName} onOpen={onOpen} />
                {i === 0 && (
                  <div className="relative z-10 -my-4 flex justify-center">
                    <button type="button" disabled={!complete || pending} onClick={() => onSave([pair[1], pair[0]])}
                      aria-label="Inverter os slots" title={complete ? 'Inverter os slots: a segunda posição vira a primeira' : 'Escolha os dois slots para inverter'}
                      className={cx('flex size-9 items-center justify-center rounded-full bg-surface text-ink shadow-soft ring-1 ring-line transition-colors enabled:hover:bg-accent enabled:hover:text-on-accent disabled:cursor-not-allowed disabled:text-muted', focusRing)}>
                      <SwapIcon />
                    </button>
                  </div>
                )}
              </div>
            ))}
            <div className="mt-auto flex flex-col gap-1 pt-2">{messages}</div>
          </aside>
          <section aria-label={`Escolha da ${slotLabels[index].toLowerCase()}`} className="flex min-h-0 flex-col">
            <div role="tablist" aria-label="O que escolher" className="flex gap-1 overflow-x-auto border-b border-line px-4 pt-3">
              {TABS.map(option => {
                const locked = option !== 'position' && option !== 'champion' && !savedSlot(index);
                return (
                  <button key={option} type="button" role="tab" aria-selected={tab === option} disabled={locked}
                    title={locked ? 'Escolha a posição e o campeão dos dois slots antes' : undefined}
                    onClick={() => onOpen({ index, tab: option })}
                    className={cx('-mb-px shrink-0 border-b-2 px-3 py-2 text-body font-semibold transition-colors',
                      tab === option ? 'border-accent text-accent-ink' : 'border-transparent text-muted enabled:hover:text-ink',
                      'disabled:cursor-not-allowed disabled:opacity-50', focusRing)}>
                    {tabLabels[option]}
                  </button>
                );
              })}
            </div>
            <div role="tabpanel" aria-label={tabLabels[tab]} className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
              {tab === 'position' && (
                <PositionChoice current={slot.position} scarce={scarce}
                  onChoose={position => chooseSlots(withPosition(pair, index, position), true)} />
              )}
              {tab === 'champion' && (
                <ChampionChoice key={index} state={state} current={slot.championId} other={pair[1 - index].championId}
                  position={slot.position} onChoose={championId => chooseSlots(withChampion(pair, index, championId), true)} />
              )}
              {needsSaved && !saved && (
                <p className="px-1 text-body text-muted">Escolha a posição e o campeão dos dois slots para mexer nos feitiços, nas runas e na skin.</p>
              )}
              {tab === 'skin' && saved && (
                <SkinChoice state={state} send={send} championId={saved.championId} current={saved.skinId}
                  onChoose={skinId => onSave(pair, { index, skinId })} />
              )}
              {tab === 'spells' && saved && (() => {
                const current: [number, number] = [saved.spell1Id, saved.spell2Id];
                const which = open.spell ?? 0;
                return (
                  <>
                    <Segmented label="Tecla do feitiço" appearance="embedded" value={which === 0 ? 'D' : 'F'}
                      options={[{ value: 'D', label: <><SpellIcon id={current[0] || null} size={20} /> Tecla D</> },
                        { value: 'F', label: <><SpellIcon id={current[1] || null} size={20} /> Tecla F</> }]}
                      onChange={key => onOpen({ index, tab: 'spells', spell: key === 'D' ? 0 : 1 })} />
                    <SpellChoice state={state} current={current[which]}
                      onChoose={spellId => onSave(pair, { index, spells: withSpell(current, which, spellId) })} />
                  </>
                );
              })()}
              {tab === 'runes' && saved && (
                <>
                  <h3 className="px-1 text-caption font-semibold uppercase tracking-[0.06em] text-muted">Use uma página salva</h3>
                  <RuneChoice state={state} onChoose={runePageId => onSave(pair, { index, runePageId })} />
                  {/* Editor compacto das runas do slot (#331), com as que ele já tem. */}
                  {state.runeCatalog && (
                    <>
                      <h3 className="px-1 pt-2 text-caption font-semibold uppercase tracking-[0.06em] text-muted">Ou edite as runas</h3>
                      <SlotRuneEditor key={`${saved.championId}:${saved.perkIds.join(',')}`} catalog={state.runeCatalog} slot={saved}
                        onSave={runes => onSave(pair, { index, runes })} />
                    </>
                  )}
                </>
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

/** Um slot na coluna da esquerda do pop-up: a arte da skin ao fundo e os botões de cada parte. */
function SlotCard({ index, slot, saved, active, tab, openSpell, state, scarce, championName, onOpen }: {
  index: 0 | 1; slot: SlotPair[number]; saved: PlayerSlot | null; active: boolean; tab: Tab | null; openSpell?: 0 | 1; state: AgentState;
  scarce: Position[]; championName: (id: number) => string; onOpen: (open: Open) => void;
}) {
  const skinId = saved?.skinId || (slot.championId ? slot.championId * 1000 : 0);
  const part = (label: string, partTab: Tab, children: ReactNode, spell?: 0 | 1) => (
    <button type="button" aria-label={`${slotLabels[index]}: ${label}`} title={label}
      aria-pressed={tab === partTab && (partTab !== 'spells' || (openSpell ?? 0) === spell)}
      onClick={event => { event.stopPropagation(); onOpen({ index, tab: partTab, spell }); }}
      className={cx('relative flex items-center justify-center rounded-control-sm bg-surface/85 p-0.5 shadow-soft backdrop-blur transition-transform hover:-translate-y-0.5', focusRing)}>
      {children}
    </button>
  );
  return (
    <div role="group" aria-label={`Slot ${slotNumerals[index]}: ${slot.championId ? championName(slot.championId) : 'sem campeão'}`}
      className={cx('relative isolate flex min-h-28 cursor-pointer flex-col justify-between overflow-hidden rounded-card p-3 transition-shadow',
        active ? 'ring-2 ring-accent shadow-lift' : 'ring-1 ring-line hover:ring-accent/50')}
      onClick={() => onOpen({ index, tab: 'champion' })}>
      {skinId > 0 && (
        <img src={assetUrl('skinSplash', skinId)} alt="" aria-hidden="true"
          className="absolute inset-0 -z-10 h-full w-full object-cover object-[center_20%]"
          onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
      )}
      <div aria-hidden="true" className="absolute inset-0 -z-10 bg-gradient-to-r from-surface via-surface/70 to-transparent" />
      <div className="flex items-center gap-2">
        <span aria-hidden="true" className={cx('flex size-6 items-center justify-center rounded-control-sm text-caption font-bold',
          active ? 'bg-accent text-on-accent' : 'bg-raised text-muted')}>{slotNumerals[index]}</span>
        <span className="truncate text-body font-semibold">{slot.championId ? championName(slot.championId) : 'Escolha um campeão'}</span>
      </div>
      <div className="flex items-center gap-1.5">
        {part(positionNames[slot.position], 'position', (
          <span className={cx('flex size-8 items-center justify-center', slot.position === 'unselected' ? 'text-muted' : 'text-ink')}>
            <PositionIcon position={slot.position} size={20} />
            {scarce.includes(slot.position) && <ScarceMark className="absolute -right-1 -top-1" />}
          </span>
        ))}
        {saved && (
          <>
            {part(`feitiço D, ${spellName(state.summonerSpellCatalog, saved.spell1Id || null)}`, 'spells',
              <SpellIcon id={saved.spell1Id || null} size={30} />, 0)}
            {part(`feitiço F, ${spellName(state.summonerSpellCatalog, saved.spell2Id || null)}`, 'spells',
              <SpellIcon id={saved.spell2Id || null} size={30} />, 1)}
            {part('runas', 'runes', <RuneIcon kind="perk" id={saved.keystoneId} size={30} />)}
            {part('skin', 'skin', <SkinThumb id={skinId} size={30} />)}
          </>
        )}
      </div>
    </div>
  );
}

function SwapIcon() {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"
      strokeLinecap="round" strokeLinejoin="round"><path d="M8 4v16M4 8l4-4 4 4M16 20V4M12 16l4 4 4-4" /></svg>
  );
}

function PositionChoice({ current, scarce, onChoose }: { current: Position; scarce: Position[]; onChoose: (position: Position) => void }) {
  return (
    <>
      <ul className="flex justify-between gap-1">
        {SLOT_POSITIONS.map(position => (
          <li key={position}>
            <button type="button" aria-pressed={current === position} onClick={() => onChoose(position)}
              aria-label={`${positionNames[position]}${scarce.includes(position) ? ' (prioritária)' : ''}`}
              className={cx('relative flex w-16 flex-col items-center gap-1 rounded-control p-2 text-caption font-semibold text-muted hover:bg-raised hover:text-ink aria-pressed:bg-accent-soft aria-pressed:text-accent-ink', focusRing)}>
              <PositionIcon position={position} size={26} />
              <span className="truncate">{positionNames[position]}</span>
              {scarce.includes(position) && <ScarceMark className="absolute right-2 top-1.5" />}
            </button>
          </li>
        ))}
      </ul>
      {scarce.length > 0 && (
        <p className="flex items-center gap-2 px-1 text-caption text-muted"><ScarceMark /> Posição prioritária: falta na fila agora.</p>
      )}
    </>
  );
}

function isLane(position: Position): position is Lane {
  return (LANES as readonly Position[]).includes(position);
}

/**
 * Grade de campeões do slot, com busca e filtro de rota (#404). O filtro começa na posição do slot, como no
 * client oficial; clicar na rota ativa tira o filtro.
 */
function ChampionChoice({ state, current, other, position, onChoose }: {
  state: AgentState; current: number; other: number; position: Position; onChoose: (championId: number) => void;
}) {
  const [query, setQuery] = useState('');
  const [lane, setLane] = useState<Lane | null>(isLane(position) ? position : null);
  const champions = slotChampions(state.championCatalog, state.playableChampions);
  const showLanes = hasLanes(state.championCatalog);
  const activeLane = showLanes ? lane : null;
  const shown = filterByLane(searchChampions(champions, query), activeLane);
  if (!state.championCatalog) return <p className="px-1 text-body text-muted">Carregando campeões…</p>;
  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <input type="search" aria-label="Buscar campeão" placeholder="Buscar campeão" value={query}
          aria-keyshortcuts="Enter Escape" title="Enter escolhe o primeiro campeão; Esc limpa."
          onChange={event => setQuery(event.target.value)}
          onKeyDown={event => {
            if (event.key === 'Escape' && query) { event.preventDefault(); event.stopPropagation(); setQuery(''); return; }
            if (event.key === 'Enter' && query && shown[0]) { event.preventDefault(); onChoose(shown[0].id); }
          }}
          className={`h-9 min-w-0 flex-1 rounded-control border border-control bg-raised px-3 text-body text-ink ${focusRing}`} />
        {showLanes && (
          <ul aria-label="Filtrar por rota" className="flex gap-1">
            {LANES.map(option => (
              <li key={option}>
                <button type="button" aria-pressed={activeLane === option} title={positionNames[option]} aria-label={positionNames[option]}
                  onClick={() => setLane(current => current === option ? null : option)}
                  className={cx('flex size-9 items-center justify-center rounded-control border border-control text-muted hover:bg-raised hover:text-ink aria-pressed:border-accent aria-pressed:bg-accent-soft aria-pressed:text-accent-ink', focusRing)}>
                  <PositionIcon position={option} size={20} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      {shown.length === 0
        ? <p className="px-1 text-body text-muted">{activeLane && !query
          ? `Nenhum dos seus campeões joga de ${positionNames[activeLane]}. Tire o filtro de rota para ver todos.`
          : 'Nenhum campeão encontrado.'}</p>
        : (
          <ul aria-label="Campeões" className="grid grid-cols-[repeat(auto-fill,minmax(5rem,1fr))] gap-1">
            {shown.map(champion => (
              <li key={champion.id}>
                <button type="button" aria-pressed={current === champion.id} onClick={() => onChoose(champion.id)}
                  title={champion.id === other ? `${champion.name} (troca com a outra posição)` : champion.name}
                  className={cx('flex w-full flex-col items-center gap-1 rounded-control p-1 text-caption text-ink hover:bg-raised aria-pressed:bg-accent-soft aria-pressed:text-accent-ink', focusRing)}>
                  <ChampionIcon id={champion.id} size={48} dim={champion.id === other} />
                  <span className="w-full truncate text-center">{champion.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
    </>
  );
}

function SpellChoice({ state, current, onChoose }: { state: AgentState; current: number; onChoose: (spellId: number) => void }) {
  const catalog = state.summonerSpellCatalog;
  // Se o arquivo de feitiços ainda não lista o modo do Jogo Dinâmico, valem os do Summoner's Rift.
  const swiftplay = spellsForQueue(catalog, 'swiftplay');
  const spells = swiftplay.length > 0 ? swiftplay : spellsForQueue(catalog, 'normalDraft');
  if (!catalog) return <p className="px-1 text-body text-muted">Carregando feitiços…</p>;
  return (
    <ul className="flex flex-wrap gap-1">
      {spells.map(spell => {
        const unlocked = spellUnlocked(spell, state.summoner?.summonerLevel);
        return (
          <li key={spell.id}>
            <button type="button" aria-pressed={current === spell.id} disabled={!unlocked} onClick={() => onChoose(spell.id)}
              title={unlocked ? spell.name : `${spell.name} (libera no nível ${spell.summonerLevel})`}
              className={cx('flex w-[4.5rem] flex-col items-center gap-1 rounded-control p-1 text-caption text-ink hover:bg-raised aria-pressed:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-40', focusRing)}>
              <SpellIcon id={spell.id} />
              <span className="w-full truncate text-center">{spell.name}</span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

/** Miniatura da skin: o recorte de cima da arte de carregamento. */
function SkinThumb({ id, size }: { id: number; size: number }) {
  return (
    <span className="block overflow-hidden rounded-full bg-raised" style={{ width: size, height: size }}>
      <img src={assetUrl('skin', id)} alt="" aria-hidden="true" className="h-full w-full object-cover object-top"
        onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
    </span>
  );
}

/** Skins da conta para o campeão do slot (#403), pedidas ao abrir; a padrão está sempre na lista. */
function SkinChoice({ state, send, championId, current, onChoose }: {
  state: AgentState; send: AgentClient['send']; championId: number; current: number; onChoose: (skinId: number) => void;
}) {
  const load = useCommand(send);
  const { run } = load;
  useEffect(() => {
    void run({ type: 'openChampionSkins', championId }, { pending: 'Carregando as skins…', failure: 'carregar as skins' });
    // `run` muda a cada render; as skins são pedidas de novo só para outro campeão.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [championId]);
  const loaded = state.championSkins?.championId === championId;
  const skins = slotSkins(state.championSkins, championId);
  if (!loaded) {
    return <p className={cx('px-1 text-body', load.feedback.status === 'error' ? 'text-danger' : 'text-muted')}>
      {load.feedback.status === 'error' ? load.feedback.message : 'Carregando as skins…'}
    </p>;
  }
  const selected = current || championId * 1000;
  return (
    <ul aria-label="Skins" className="grid grid-cols-[repeat(auto-fill,minmax(7rem,1fr))] gap-2">
      {skins.map(skin => (
        <li key={skin.id}>
          <button type="button" aria-pressed={selected === skin.id} onClick={() => onChoose(skin.id)}
            aria-label={skin.isBase ? `${skin.name} (padrão)` : skin.name} title={skin.name}
            className={cx('flex w-full flex-col gap-1 rounded-control border-2 border-transparent p-1 text-left hover:bg-raised aria-pressed:border-accent aria-pressed:bg-accent-soft', focusRing)}>
            <span className="block aspect-[308/560] w-full overflow-hidden rounded-control-sm bg-raised">
              <img src={assetUrl('skin', skin.id)} alt="" loading="lazy" className="h-full w-full object-cover"
                onError={event => { event.currentTarget.style.visibility = 'hidden'; }} />
            </span>
            <span className="line-clamp-2 px-0.5 text-caption font-semibold text-ink">{skin.name}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}

const runeButton = cx('flex items-center justify-center rounded-full p-0.5 transition-colors hover:bg-raised',
  'aria-pressed:bg-accent-soft aria-pressed:ring-2 aria-pressed:ring-accent', focusRing);

/**
 * Editor compacto das runas de um slot (#331): só os ícones, com o nome no
 * hover (e para o leitor de tela), e o botão Salvar no fim. Começa com as
 * runas que o slot já tem; sem elas, numa página nova.
 */
function SlotRuneEditor({ catalog, slot, onSave }: { catalog: RuneCatalog; slot: PlayerSlot | undefined; onSave: (runes: SlotRunesInput) => void }) {
  const [draft, setDraft] = useState<RuneDraft>(() => slot && slot.perkIds.length
    ? draftFromPage(catalog, { name: 'Slot', primaryStyleId: slot.runeStyleId, subStyleId: slot.runeSubStyleId, selectedPerkIds: slot.perkIds })
    : newDraft(catalog, 'Slot'));
  const primary = findStyle(catalog, draft.primaryStyleId);
  const sub = findStyle(catalog, draft.subStyleId);
  const problem = draftProblem(draft);
  const perkName = (id: number) => catalog.perks.find(p => p.id === id)?.name ?? `Runa ${id}`;

  function row(perks: number[], label: string, selected: (id: number) => boolean, choose: (id: number) => void, size: number) {
    const anySelected = perks.some(selected);
    return (
      <div role="group" aria-label={label} className="flex flex-wrap gap-1">
        {perks.map(id => (
          <button key={id} type="button" aria-pressed={selected(id)} aria-label={perkName(id)} title={perkName(id)}
            onClick={() => choose(id)} className={runeButton}>
            <RuneIcon kind="perk" id={id} size={size} className={cx(anySelected && !selected(id) && 'opacity-50 grayscale')} />
          </button>
        ))}
      </div>
    );
  }
  function styles(options: RuneStyle[], selected: number, label: string, choose: (id: number) => void) {
    return (
      <div role="group" aria-label={label} className="flex flex-wrap gap-1">
        {options.map(style => (
          <button key={style.id} type="button" aria-pressed={style.id === selected} aria-label={style.name} title={style.name}
            onClick={() => choose(style.id)} className={runeButton}>
            <RuneIcon kind="perkStyle" id={style.id} size={22} className={cx(style.id !== selected && 'opacity-60')} />
          </button>
        ))}
      </div>
    );
  }

  return (
    <form aria-label="Editar runas" className="flex flex-col gap-2"
      onSubmit={event => { event.preventDefault(); if (!problem) onSave(slotRunes(draft)); }}>
      <div className="grid grid-cols-2 gap-3">
        <div className="flex min-w-0 flex-col gap-1.5">
          {styles(catalog.styles, draft.primaryStyleId, 'Árvore primária', id => setDraft(withPrimary(catalog, draft, id)))}
          {primarySlots(primary).map((slot, index) => (
            <div key={`p${index}`}>{row(slot.perks, slot.kind === 'keystone' ? 'Runa principal' : `Primária, linha ${index}`,
              id => draft.primary[index] === id, id => setDraft(withPrimaryPerk(draft, index, id)), slot.kind === 'keystone' ? 34 : 26)}</div>
          ))}
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          {styles(allowedSubStyles(catalog, primary), draft.subStyleId, 'Árvore secundária', id => setDraft(withSubStyle(draft, id)))}
          {subSlots(sub).map((slot, index) => (
            <div key={`s${index}`}>{row(slot.perks, `Secundária, linha ${index + 1}`,
              id => draft.sub.some(pick => pick.row === index && pick.perkId === id), id => setDraft(withSubPerk(draft, index, id)), 24)}</div>
          ))}
          {statSlots(primary).map((slot, index) => (
            <div key={`f${index}`}>{row(slot.perks, slot.label || `Fragmento ${index + 1}`,
              id => draft.stats[index] === id, id => setDraft(withStatPerk(draft, index, id)), 18)}</div>
          ))}
        </div>
      </div>
      <div className="flex items-center justify-between gap-2 border-t border-line pt-2">
        <p className={cx('min-w-0 text-caption', problem ? 'text-warn' : 'text-muted')}>{problem ?? 'Passe o mouse para ver o nome.'}</p>
        <button type="submit" disabled={!!problem}
          className={cx('inline-flex h-9 shrink-0 items-center rounded-full bg-accent px-4 text-caption font-semibold text-on-accent disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted', focusRing)}>
          Salvar
        </button>
      </div>
    </form>
  );
}

/** Runas do rascunho no formato do comando, na ordem da página. */
function slotRunes(draft: RuneDraft): SlotRunesInput {
  const { primaryStyleId, subStyleId, selectedPerkIds } = draftToInput(draft);
  return { perkIds: selectedPerkIds, styleId: primaryStyleId, subStyleId };
}

function RuneChoice({ state, onChoose }: { state: AgentState; onChoose: (pageId: number) => void }) {
  const pages = (state.runePages?.pages ?? []).filter(page => !page.isTemporary && page.isValid);
  if (!state.runePages) return <p className="px-1 text-body text-muted">Carregando páginas de runas…</p>;
  if (pages.length === 0) return <p className="px-1 text-body text-muted">Nenhuma página de runas completa. As runas recomendadas continuam.</p>;
  return (
    <ul className="flex max-h-64 flex-col gap-1 overflow-y-auto">
      {pages.map(page => (
        <li key={page.id}>
          <button type="button" onClick={() => onChoose(page.id)}
            className={cx('flex w-full items-center gap-3 rounded-control px-2 py-1.5 text-left text-body font-medium text-ink hover:bg-raised', focusRing)}>
            <span className="relative shrink-0">
              <RuneIcon kind="perk" id={page.selectedPerkIds[0] ?? 0} size={28} />
              <RuneIcon kind="perkStyle" id={page.subStyleId} size={14} className="absolute -bottom-1 -right-1 border-2 border-surface" />
            </span>
            <span className="min-w-0 flex-1 truncate">{page.name}</span>
          </button>
        </li>
      ))}
    </ul>
  );
}
