import { useEffect, useRef, useState } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient } from '../agent/client';
import { SMITE_ID, smiteRule, spellName, spellsForChampSelect, spellUnlocked, withSpell } from '../agent/summonerSpells';
import { useCommand } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { GameQueue } from '../bindings/GameQueue';
import type { Lobby } from '../bindings/Lobby';
import type { SummonerSpellCatalog } from '../bindings/SummonerSpellCatalog';

export function SpellIcon({ id, size = 32 }: { id: number | null; size?: number }) {
  return (
    <span aria-hidden="true" className="inline-block shrink-0 overflow-hidden rounded-control-sm bg-raised" style={{ width: size, height: size }}>
      {id ? <img src={assetUrl('summonerSpell', id)} alt="" width={size} height={size}
        onError={(event) => { event.currentTarget.style.visibility = 'hidden'; }} /> : null}
    </span>
  );
}

type Props = {
  champSelect: ChampSelect;
  catalog: SummonerSpellCatalog | null;
  summonerLevel: number | null;
  /** Filas disponíveis, para achar o modo de jogo das filas fora das fixas (#203). */
  queues?: GameQueue[];
  /** Lobby da seleção, para o modo de jogo da personalizada. */
  lobby?: Lobby | null;
  send: AgentClient['send'];
};

const slotNames = ['Feitiço 1 (D)', 'Feitiço 2 (F)'] as const;

/** Escolha dos dois feitiços de invocador do jogador logado (#24). */
export function SummonerSpellsPicker({ champSelect, catalog, summonerLevel, queues = [], lobby = null, send }: Props) {
  const [open, setOpen] = useState<0 | 1 | null>(null);
  const command = useCommand(send);
  const slotButtons = useRef<(HTMLButtonElement | null)[]>([]);
  const list = useRef<HTMLUListElement>(null);
  // Aberto pelo mouse (#475), o foco fica onde está.
  const [byHover, setByHover] = useState(false);
  // Ao abrir pelo clique ou teclado, o foco vai para o feitiço atual; Esc fecha e devolve o foco ao botão do espaço.
  useEffect(() => {
    if (open === null || byHover) return;
    list.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]:not(:disabled), button:not(:disabled)')?.focus();
  }, [open, byHover]);
  function close(slot: 0 | 1) {
    setOpen(null);
    if (!byHover) slotButtons.current[slot]?.focus();
  }
  const me = champSelect.myTeam.find(p => p.cellId === champSelect.localPlayerCellId);
  if (!me) return null;
  const current: [number, number] = [me.spell1Id ?? 0, me.spell2Id ?? 0];
  const disabled = champSelect.timer.phase === 'GAME_STARTING' || command.pending || !catalog;
  // Na ranqueada (#480), só o caçador vê o Golpear, e ele não pode tirá-lo (só trocar de tecla).
  const rule = smiteRule(champSelect);
  const spells = spellsForChampSelect(catalog, champSelect, queues, lobby).filter(spell => rule !== 'forbidden' || spell.id !== SMITE_ID);
  const dropsSmite = (slot: 0 | 1, spellId: number) => rule === 'required' && !withSpell(current, slot, spellId).includes(SMITE_ID);
  function choose(slot: 0 | 1, spellId: number) {
    if (disabled) return;
    close(slot);
    const [spell1Id, spell2Id] = withSpell(current, slot, spellId);
    if (spell1Id === current[0] && spell2Id === current[1]) return;
    void command.run({ type: 'setSummonerSpells', spell1Id, spell2Id },
      { pending: 'Trocando os feitiços…', failure: 'trocar os feitiços' });
  }
  return (
    <section aria-label="Seus feitiços" className="flex flex-col gap-2">
      <h3 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Feitiços</h3>
      {/* Passar o mouse num feitiço abre a escolha para cima, por cima da tela, numa grade 3x3 (#475). */}
      {/* Os dois feitiços lado a lado, sempre do mesmo tamanho, qualquer que seja o nome (#475). */}
      <div className="grid grid-cols-2 gap-2">
        {slotNames.map((label, index) => {
          const slot = index as 0 | 1;
          return (
            <div key={label} className="relative min-w-0"
              onMouseEnter={() => { if (!disabled) { setByHover(true); setOpen(slot); } }}
              onMouseLeave={() => { if (byHover && open === slot) setOpen(null); }}>
              <button ref={element => { slotButtons.current[slot] = element; }} type="button" aria-label={`${label}: ${spellName(catalog, current[slot] || null)}`}
                aria-expanded={open === slot} disabled={disabled}
                onClick={() => { setByHover(false); setOpen(open === slot && !byHover ? null : slot); }}
                className="flex w-full min-w-0 items-center gap-1.5 rounded-control border border-control bg-surface px-1.5 py-1 text-ink aria-expanded:border-accent aria-expanded:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-50">
                <SpellIcon id={current[slot] || null} size={26} />
                <span className="min-w-0 truncate text-body">{spellName(catalog, current[slot] || null)}</span>
              </button>
              {open === slot && (
                // O espaço em branco embaixo da grade faz parte da área do mouse: ir do botão até a grade não fecha.
                <div className="absolute bottom-full left-0 z-30 pb-2">
                  <ul ref={list} aria-label={`Escolher ${slotNames[slot]}`}
                    className="grid w-max grid-cols-3 gap-1 rounded-card border border-edge bg-surface p-2 shadow-lift animate-fade-up"
                    onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); close(slot); } }}>
                    {spells.map(spell => {
                      const unlocked = spellUnlocked(spell, summonerLevel);
                      const locked = dropsSmite(slot, spell.id);
                      return (
                        <li key={spell.id}>
                          <button type="button" aria-pressed={current[slot] === spell.id} disabled={disabled || !unlocked || locked}
                            title={locked ? `${spell.name} (na ranqueada, o caçador precisa ficar com o Golpear)`
                              : unlocked ? spell.name : `${spell.name} (libera no nível ${spell.summonerLevel})`}
                            onClick={() => choose(slot, spell.id)}
                            className="flex w-20 flex-col items-center gap-1 rounded-control border-2 border-transparent p-1 text-caption text-ink hover:bg-raised aria-pressed:border-accent aria-pressed:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-40">
                            <SpellIcon id={spell.id} />
                            <span className="w-full truncate text-center">{spell.name}</span>
                          </button>
                        </li>
                      );
                    })}
                  </ul>
                </div>
              )}
            </div>
          );
        })}
      </div>
      {!catalog && <p className="text-muted">Carregando feitiços…</p>}
      {rule === 'required' && <p className="text-caption text-muted">Na ranqueada, o caçador fica com o Golpear; dá para trocar a tecla.</p>}
      {command.feedback.status !== 'idle' && (
        <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
          className={command.feedback.status === 'error' ? 'text-danger' : 'text-muted'}>{command.feedback.message}</p>
      )}
    </section>
  );
}
