import { useEffect, useRef, useState } from 'react';
import { assetUrl } from '../agent/assets';
import type { AgentClient } from '../agent/client';
import { spellName, spellsForChampSelect, spellUnlocked, withSpell } from '../agent/summonerSpells';
import { useCommand } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { GameQueue } from '../bindings/GameQueue';
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
  send: AgentClient['send'];
};

const slotNames = ['Feitiço 1 (D)', 'Feitiço 2 (F)'] as const;

/** Escolha dos dois feitiços de invocador do jogador logado (#24). */
export function SummonerSpellsPicker({ champSelect, catalog, summonerLevel, queues = [], send }: Props) {
  const [open, setOpen] = useState<0 | 1 | null>(null);
  const command = useCommand(send);
  const slotButtons = useRef<(HTMLButtonElement | null)[]>([]);
  const list = useRef<HTMLUListElement>(null);
  // Ao abrir, o foco vai para o feitiço atual; Esc fecha e devolve o foco ao botão do espaço.
  useEffect(() => {
    if (open === null) return;
    list.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"]:not(:disabled), button:not(:disabled)')?.focus();
  }, [open]);
  function close(slot: 0 | 1) {
    setOpen(null);
    slotButtons.current[slot]?.focus();
  }
  const me = champSelect.myTeam.find(p => p.cellId === champSelect.localPlayerCellId);
  if (!me) return null;
  const current: [number, number] = [me.spell1Id ?? 0, me.spell2Id ?? 0];
  const disabled = champSelect.timer.phase === 'GAME_STARTING' || command.pending || !catalog;
  const spells = spellsForChampSelect(catalog, champSelect, queues);
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
      <div className="flex flex-wrap gap-2">
        {slotNames.map((label, index) => {
          const slot = index as 0 | 1;
          return (
            <button key={label} ref={element => { slotButtons.current[slot] = element; }} type="button" aria-label={`${label}: ${spellName(catalog, current[slot] || null)}`}
              aria-expanded={open === slot} disabled={disabled}
              onClick={() => setOpen(open === slot ? null : slot)}
              className="flex items-center gap-2 rounded-control border border-control bg-surface px-2 py-1 text-ink aria-expanded:border-accent aria-expanded:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-50">
              <SpellIcon id={current[slot] || null} />
              <span className="text-body">{spellName(catalog, current[slot] || null)}</span>
            </button>
          );
        })}
      </div>
      {!catalog && <p className="text-muted">Carregando feitiços…</p>}
      {open !== null && (
        <ul ref={list} aria-label={`Escolher ${slotNames[open]}`} className="flex flex-wrap gap-1 rounded-card border border-line bg-surface p-1"
          onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); close(open); } }}>
          {spells.map(spell => {
            const unlocked = spellUnlocked(spell, summonerLevel);
            return (
              <li key={spell.id}>
                <button type="button" aria-pressed={current[open] === spell.id} disabled={disabled || !unlocked}
                  title={unlocked ? spell.name : `${spell.name} (libera no nível ${spell.summonerLevel})`}
                  onClick={() => choose(open, spell.id)}
                  className="flex w-20 flex-col items-center gap-1 rounded-control border-2 border-transparent p-1 text-caption text-ink hover:bg-raised aria-pressed:border-accent aria-pressed:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-40">
                  <SpellIcon id={spell.id} />
                  <span className="w-full truncate text-center">{spell.name}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {command.feedback.status !== 'idle' && (
        <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
          className={command.feedback.status === 'error' ? 'text-danger' : 'text-muted'}>{command.feedback.message}</p>
      )}
    </section>
  );
}
