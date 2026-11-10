import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { AgentClient } from '../agent/client';
import { searchChampions } from '../agent/champSelect';
import { positionNames } from '../agent/positions';
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
import { ScarceMark } from './ScarceMark';
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

type Open = { index: 0 | 1; kind: 'champion' | 'position' | 'spells' | 'runes' | 'skin'; spell?: 0 | 1 };
const slotLabels = ['Primeira posição', 'Segunda posição'] as const;
const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';

/**
 * Posições e campeões do Jogo Dinâmico (#218) no card do jogador no lobby: dois slots, cada um com a posição, o
 * campeão, os feitiços e as runas. No card do jogador logado cada parte abre a sua escolha; nos outros cards é só
 * para ver. A escolha vai para o agente quando os dois slots têm posição e campeão.
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
  const pair = draft.slots;
  const scarce = state.lobby?.scarcePositions ?? [];
  const catalog = state.championCatalog;
  const championName = (id: number) => catalog?.champions.find(c => c.id === id)?.name ?? (id ? `Campeão ${id}` : 'Escolher campeão');

  async function save(next: SlotPair, extras?: SlotExtras) {
    setDraft({ key: confirmedKey, slots: next });
    if (!draftComplete(next) || (!extras && sameDraft(next, confirmed))) return;
    await command.run(slotsCommand(next, extras), { pending: 'Salvando posições e campeões…', failure: 'salvar as posições e os campeões' });
    // O agente publica o lobby antes de responder: a tela volta ao que ele confirmou.
    setDraft(current => ({ ...current, key: '' }));
  }
  function choose(next: SlotPair, extras?: SlotExtras) {
    setOpen(null);
    void save(next, extras);
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
          const saved = member.playerSlots[index];
          const savedHere = !!saved && saved.championId === slot.championId && saved.position === slot.position && slot.championId > 0;
          const isOpen = (kind: Open['kind'], spell?: 0 | 1) => open?.index === index && open.kind === kind && open.spell === spell;
          const toggle = (kind: Open['kind'], spell?: 0 | 1) => setOpen(isOpen(kind, spell) ? null : { index, kind, spell });
          return (
            <li key={index} className="flex items-center gap-1">
              <SlotButton editable={editable} label={`${slotLabels[index]}: ${slot.championId ? championName(slot.championId) : 'sem campeão'}`}
                expanded={isOpen('champion')} disabled={command.pending} onClick={() => toggle('champion')}>
                <span className="relative block">
                  <ChampionIcon id={slot.championId || null} size={64} />
                  {!slot.championId && <span aria-hidden="true" className="absolute inset-0 flex items-center justify-center text-title text-muted">?</span>}
                </span>
              </SlotButton>
              <span className="flex flex-col items-start gap-1">
                <SlotButton editable={editable} label={`${slotLabels[index]}: ${positionNames[slot.position]}`}
                  expanded={isOpen('position')} disabled={command.pending} onClick={() => toggle('position')}>
                  <span className={cx('relative flex size-8 items-center justify-center rounded-control-sm bg-raised',
                    slot.position === 'unselected' ? 'text-muted' : 'text-ink')}>
                    <PositionIcon position={slot.position} size={20} />
                    {scarce.includes(slot.position) && <ScarceMark className="absolute -right-1 -top-1" />}
                  </span>
                </SlotButton>
                {savedHere && (
                  <span className="flex items-center gap-0.5">
                    {([0, 1] as const).map(spell => (
                      <SlotButton key={spell} editable={editable} disabled={command.pending} expanded={isOpen('spells', spell)}
                        label={`${slotLabels[index]}, feitiço ${spell === 0 ? 'D' : 'F'}: ${spellName(state.summonerSpellCatalog, (spell === 0 ? saved.spell1Id : saved.spell2Id) || null)}`}
                        onClick={() => toggle('spells', spell)}>
                        <SpellIcon id={(spell === 0 ? saved.spell1Id : saved.spell2Id) || null} size={26} />
                      </SlotButton>
                    ))}
                    <SlotButton editable={editable} disabled={command.pending} label={`${slotLabels[index]}: runas`}
                      expanded={isOpen('runes')} onClick={() => toggle('runes')}>
                      <RuneIcon kind="perk" id={saved.keystoneId} size={26} />
                    </SlotButton>
                    {/* Skin do campeão no slot (#403). */}
                    <SlotButton editable={editable} disabled={command.pending} label={`${slotLabels[index]}: skin`}
                      expanded={isOpen('skin')} onClick={() => toggle('skin')}>
                      <SkinThumb id={saved.skinId || saved.championId * 1000} size={26} />
                    </SlotButton>
                  </span>
                )}
              </span>
            </li>
          );
        })}
      </ul>
      {notes ? messages && createPortal(messages, notes) : messages}
      {editable && open && (
        <Popover title={popoverTitle(open)} onClose={() => setOpen(null)}>
          {open.kind === 'position' && (
            <PositionChoice current={pair[open.index].position} scarce={scarce}
              onChoose={position => choose(withPosition(pair, open.index, position))} />
          )}
          {open.kind === 'champion' && (
            <ChampionChoice state={state} current={pair[open.index].championId} other={pair[1 - open.index].championId}
              onChoose={championId => choose(withChampion(pair, open.index, championId))} />
          )}
          {open.kind === 'spells' && member.playerSlots[open.index] && (() => {
            const saved = member.playerSlots[open.index];
            const current: [number, number] = [saved.spell1Id, saved.spell2Id];
            return (
              <SpellChoice state={state} current={current[open.spell ?? 0]}
                onChoose={spellId => choose(pair, { index: open.index, spells: withSpell(current, open.spell ?? 0, spellId) })} />
            );
          })()}
          {open.kind === 'runes' && (
            <>
              {/* Editor compacto das runas do slot (#331), com as que ele já tem. */}
              {state.runeCatalog && (
                <SlotRuneEditor catalog={state.runeCatalog} slot={member.playerSlots[open.index]}
                  onSave={runes => choose(pair, { index: open.index, runes })} />
              )}
              <h4 className="px-1 text-caption font-semibold uppercase tracking-[0.06em] text-muted">Ou use uma página salva</h4>
              <RuneChoice state={state} onChoose={runePageId => choose(pair, { index: open.index, runePageId })} />
            </>
          )}
          {open.kind === 'skin' && member.playerSlots[open.index] && (
            <SkinChoice state={state} send={send} championId={member.playerSlots[open.index].championId}
              current={member.playerSlots[open.index].skinId}
              onChoose={skinId => choose(pair, { index: open.index, skinId })} />
          )}
        </Popover>
      )}
    </div>
  );
}

function popoverTitle(open: Open): string {
  const slot = slotLabels[open.index].toLowerCase();
  switch (open.kind) {
    case 'position': return `Escolher a ${slot}`;
    case 'champion': return `Campeão da ${slot}`;
    case 'spells': return `Feitiço ${open.spell === 1 ? 'F' : 'D'} da ${slot}`;
    case 'runes': return `Runas da ${slot}`;
    case 'skin': return `Skin da ${slot}`;
  }
}

function SlotButton({ editable, label, expanded, disabled, onClick, children }: {
  editable: boolean; label: string; expanded: boolean; disabled: boolean; onClick: () => void; children: ReactNode;
}) {
  if (!editable) return <span role="img" aria-label={label} title={label}>{children}</span>;
  return (
    <button type="button" aria-label={label} title={label} aria-haspopup="dialog" aria-expanded={expanded} disabled={disabled}
      onClick={onClick}
      className={cx('rounded-control-sm transition-transform enabled:hover:-translate-y-0.5 aria-expanded:ring-2 aria-expanded:ring-accent disabled:cursor-wait',
        focusRing)}>
      {children}
    </button>
  );
}

function Popover({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const titleId = useId();
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; });
  useEffect(() => {
    const first = ref.current?.querySelector<HTMLElement>('input, button[aria-pressed="true"], button:not(:disabled)');
    (first ?? ref.current)?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') closeRef.current(); }
    function onPointer(event: PointerEvent) {
      const target = event.target as Element | null;
      // Um clique noutro botão de slot troca a escolha aberta pelo próprio botão.
      if (ref.current && !ref.current.contains(target) && !target?.closest?.('[aria-haspopup="dialog"]')) closeRef.current();
    }
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  }, []);
  return (
    <div ref={ref} role="dialog" aria-labelledby={titleId} tabIndex={-1}
      className="absolute right-0 top-full z-30 mt-2 flex w-[min(26rem,calc(100vw-3rem))] flex-col gap-3 rounded-panel bg-surface card p-3 text-ink shadow-lift animate-fade-up focus-visible:outline-none">
      <h3 id={titleId} className="px-1 text-section font-semibold">{title}</h3>
      {children}
    </div>
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

function ChampionChoice({ state, current, other, onChoose }: {
  state: AgentState; current: number; other: number; onChoose: (championId: number) => void;
}) {
  const [query, setQuery] = useState('');
  const champions = slotChampions(state.championCatalog, state.playableChampions);
  const shown = searchChampions(champions, query);
  if (!state.championCatalog) return <p className="px-1 text-body text-muted">Carregando campeões…</p>;
  return (
    <>
      <input type="search" aria-label="Buscar campeão" placeholder="Buscar campeão" value={query}
        onChange={event => setQuery(event.target.value)}
        className={`h-9 rounded-control border border-control bg-raised px-3 text-body text-ink ${focusRing}`} />
      {shown.length === 0
        ? <p className="px-1 text-body text-muted">Nenhum campeão encontrado.</p>
        : (
          <ul aria-label="Campeões" className="grid max-h-72 grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-1 overflow-y-auto pr-1">
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
    <ul aria-label="Skins" className="grid max-h-80 grid-cols-[repeat(auto-fill,minmax(5.5rem,1fr))] gap-2 overflow-y-auto pr-1">
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

/** Linha de runas em grade: cada ícone no centro de uma parte igual da largura. */
const runeRow = 'grid justify-items-center gap-1';
const columns = (count: number) => ({ gridTemplateColumns: `repeat(${Math.max(count, 1)}, minmax(0, 1fr))` });

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
      <div role="group" aria-label={label} className={runeRow} style={columns(perks.length)}>
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
      <div role="group" aria-label={label} className={runeRow} style={columns(options.length)}>
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
      {/* Duas colunas da mesma altura, como no client oficial: a primária à
          esquerda e a secundária com os fragmentos à direita. Cada linha divide
          a largura em partes iguais, e as linhas se espalham na altura, para
          os ícones formarem uma grade e as colunas, dois retângulos (#401). */}
      <div className="grid grid-cols-2">
        <div className="flex min-w-0 flex-col justify-between gap-1.5 pr-3">
          {styles(catalog.styles, draft.primaryStyleId, 'Árvore primária', id => setDraft(withPrimary(catalog, draft, id)))}
          {primarySlots(primary).map((slot, index) => (
            <div key={`p${index}`}>{row(slot.perks, slot.kind === 'keystone' ? 'Runa principal' : `Primária, linha ${index}`,
              id => draft.primary[index] === id, id => setDraft(withPrimaryPerk(draft, index, id)), slot.kind === 'keystone' ? 34 : 26)}</div>
          ))}
        </div>
        <div className="flex min-w-0 flex-col justify-between gap-1.5 border-l border-line pl-3">
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
