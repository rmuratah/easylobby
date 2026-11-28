import { useState } from 'react';
import type { AgentState } from '../bindings/AgentState';
import { assetUrl } from '../agent/assets';
import {
  advancedCategories, bestColumns, cellText, runeResults,
  type AdvancedColumn, type AdvancedRow,
} from '../agent/advancedStats';
import { Avatar } from './base';
import { cx } from './base/cx';
import { GameIcon } from './GameIcons';

type Props = {
  columns: AdvancedColumn[];
  state: AgentState;
  localPuuid: string;
  onOpenProfile?: (puuid: string) => void;
};

function championName(state: AgentState, championId: number): string {
  return state.championCatalog?.champions.find(c => c.id === championId)?.name ?? `Campeão ${championId}`;
}

const isMe = (column: AdvancedColumn, localPuuid: string) => localPuuid !== '' && column.player.puuid === localPuuid;

/** Campeão e nick; o nick abre o perfil, menos o do próprio jogador (#585). */
function PlayerName({ column, state, me, onOpenProfile, align = 'center' }: {
  column: AdvancedColumn; state: AgentState; me: boolean; onOpenProfile?: (puuid: string) => void; align?: 'center' | 'left';
}) {
  const p = column.player;
  const champion = championName(state, p.championId);
  const riotId = p.tagLine ? `${p.gameName}#${p.tagLine}` : p.gameName;
  const name = p.gameName || champion;
  const open = onOpenProfile && p.puuid !== '' && !me ? () => onOpenProfile(p.puuid) : undefined;
  return (
    <span className={cx('flex min-w-0 gap-1.5', align === 'center' ? 'flex-col items-center' : 'items-center')}>
      <Avatar name={champion} src={assetUrl('champion', p.championId)} size={30} shape="rounded" />
      <span className={cx('min-w-0 max-w-full', align === 'center' ? 'text-center' : 'text-left')}>
        {open
          ? <button type="button" onClick={open} title={riotId} aria-label={`Ver perfil de ${riotId || champion}`}
            className="block max-w-full truncate rounded-control-sm px-0.5 font-semibold text-ink hover:underline focus-visible:outline-2 focus-visible:outline-accent">
            {name}
          </button>
          : <span className="block truncate font-semibold text-ink" title={riotId}>{name}</span>}
        <span className="block truncate font-normal text-muted">{champion}</span>
      </span>
    </span>
  );
}

/**
 * Estatísticas (#586): uma coluna por jogador, o time do jogador logado à
 * esquerda, e uma linha por estatística, agrupadas em categorias que abrem e
 * fecham. O melhor valor da linha fica em destaque.
 */
export function AdvancedStatsTable({ columns, state, localPuuid, onOpenProfile }: Props) {
  const [closed, setClosed] = useState(() => new Set(advancedCategories.filter(c => c.collapsed).map(c => c.title)));
  const toggle = (title: string) => setClosed(prev => {
    const next = new Set(prev);
    if (!next.delete(title)) next.add(title);
    return next;
  });
  const span = columns.length + 1;
  return (
    <div className="overflow-x-auto rounded-card bg-raised">
      <table aria-label="Estatísticas da partida" className="w-full min-w-[52rem] table-fixed text-caption">
        <colgroup><col className="w-52" />{columns.map((_, i) => <col key={i} />)}</colgroup>
        <thead>
          <tr>
            <th scope="col" className="sticky left-0 bg-raised px-3 py-2 text-left font-semibold text-muted">Estatística</th>
            {columns.map((c, i) => (
              <th key={i} scope="col" className={cx('px-1 py-2 align-bottom font-normal',
                c.firstOfTeam && i > 0 && 'border-l-2 border-line', isMe(c, localPuuid) && 'bg-accent-soft')}>
                <PlayerName column={c} state={state} me={isMe(c, localPuuid)} onOpenProfile={onOpenProfile} />
              </th>
            ))}
          </tr>
        </thead>
        {advancedCategories.map(category => {
          const open = !closed.has(category.title);
          return (
            <tbody key={category.title}>
              <tr className="border-t border-line">
                <th colSpan={span} scope="colgroup" className="p-0 text-left">
                  <button type="button" aria-expanded={open} onClick={() => toggle(category.title)}
                    className="flex w-full items-center gap-2 px-3 py-2 text-body font-semibold text-ink hover:bg-surface focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent">
                    <span aria-hidden="true" className={cx('inline-block w-3 text-muted transition-transform', open && 'rotate-90')}>›</span>
                    {category.title}
                  </button>
                </th>
              </tr>
              {open && category.rows.map(row => <StatRow key={row.label} row={row} columns={columns} localPuuid={localPuuid} />)}
            </tbody>
          );
        })}
      </table>
    </div>
  );
}

function StatRow({ row, columns, localPuuid }: { row: AdvancedRow; columns: AdvancedColumn[]; localPuuid: string }) {
  const best = bestColumns(row, columns);
  return (
    <tr className="hover:bg-surface/60">
      <th scope="row" className="sticky left-0 truncate bg-raised py-1 pl-8 pr-2 text-left font-normal text-muted" title={row.label}>{row.label}</th>
      {columns.map((c, i) => (
        <td key={i} className={cx('px-1 py-1 text-center tabular-nums',
          c.firstOfTeam && i > 0 && 'border-l-2 border-line', isMe(c, localPuuid) && 'bg-accent-soft',
          best.has(i) ? 'font-semibold text-accent-ink' : 'text-ink')}>
          {cellText(row, c)}
        </td>
      ))}
    </tr>
  );
}

/**
 * Gráficos (#586): escolha uma estatística e veja uma barra por jogador, como
 * na aba Gráficos do client oficial.
 */
export function AdvancedCharts({ columns, state, localPuuid, onOpenProfile }: Props) {
  const rows = advancedCategories.flatMap(category => category.rows.map(row => ({ category: category.title, row })));
  const [selected, setSelected] = useState(rows[0]?.row.label ? `${rows[0].category}/${rows[0].row.label}` : '');
  const current = rows.find(r => `${r.category}/${r.row.label}` === selected) ?? rows[0];
  if (!current) return null;
  const values = columns.map(current.row.value);
  const max = Math.max(...values, 0);
  return (
    <div className="grid gap-3 rounded-card bg-raised p-3 @min-[48rem]:grid-cols-[15rem_1fr]">
      <nav aria-label="Estatística do gráfico" className="flex max-h-[28rem] flex-col gap-2 overflow-y-auto pr-1">
        {advancedCategories.map(category => (
          <div key={category.title} role="group" aria-label={category.title}>
            <p className="px-2 pb-0.5 text-caption font-semibold uppercase tracking-wide text-muted">{category.title}</p>
            {category.rows.map(row => {
              const key = `${category.title}/${row.label}`;
              return (
                <button key={key} type="button" aria-pressed={key === selected} onClick={() => setSelected(key)}
                  className={cx('block w-full truncate rounded-control-sm px-2 py-1 text-left text-caption',
                    'focus-visible:outline-2 focus-visible:outline-accent',
                    key === selected ? 'bg-accent-soft font-semibold text-accent-ink' : 'text-ink hover:bg-surface')}>
                  {row.label}
                </button>
              );
            })}
          </div>
        ))}
      </nav>
      <figure className="m-0 flex flex-col gap-1.5">
        <figcaption className="pb-1 text-body font-semibold text-ink">{current.category} · {current.row.label}</figcaption>
        {columns.map((c, i) => {
          const me = isMe(c, localPuuid);
          const ally = c.teamId === columns[0]?.teamId;
          const width = max > 0 ? Math.max(0, (values[i] ?? 0) / max) * 100 : 0;
          return (
            <div key={i} className={cx('grid grid-cols-[11rem_1fr] items-center gap-3 text-caption', c.firstOfTeam && i > 0 && 'mt-3')}>
              <PlayerName column={c} state={state} me={me} onOpenProfile={onOpenProfile} align="left" />
              <div className="flex items-center gap-2">
                <div className="h-4 flex-1 overflow-hidden rounded-full bg-surface">
                  <div className={cx('h-full rounded-full', ally ? 'bg-accent' : 'bg-danger', me && 'ring-2 ring-accent-ink')}
                    style={{ width: `${width}%` }} />
                </div>
                <span className="w-16 text-right font-semibold tabular-nums text-ink">{cellText(current.row, c)}</span>
              </div>
            </div>
          );
        })}
      </figure>
    </div>
  );
}

/**
 * Runas (#586): as seis runas de cada jogador com as estatísticas delas no
 * fim da partida (dano causado, cura...), como na aba Runas do client
 * oficial. Só aparece nos modos com runas.
 */
export function AdvancedRunes({ columns, state, localPuuid, onOpenProfile }: Props) {
  const perks = state.runeCatalog?.perks ?? [];
  // Aliados primeiro, depois os inimigos (#685). Com duas colunas, cada time
  // fica preso à sua coluna e o fluxo "dense" põe o 1º inimigo na 1ª linha,
  // ao lado do 1º aliado; com uma coluna, os aliados vêm antes dos inimigos.
  const allyTeam = columns[0]?.teamId;
  const ordered = [...columns.filter(c => c.teamId === allyTeam), ...columns.filter(c => c.teamId !== allyTeam)];
  return (
    <div className="grid gap-2 @min-[56rem]:grid-flow-row-dense @min-[56rem]:grid-cols-2">
      {ordered.map((c, i) => {
        const me = isMe(c, localPuuid);
        const ally = c.teamId === allyTeam;
        const runes = runeResults(c.stats, perks);
        return (
          <section key={i} aria-label={`Runas de ${c.player.gameName || championName(state, c.player.championId)}`}
            data-team={ally ? 'aliado' : 'inimigo'}
            className={cx('flex flex-col gap-2 rounded-card border-l-4 p-3', me ? 'bg-accent-soft' : 'bg-raised',
              ally ? 'border-accent @min-[56rem]:col-start-1' : 'border-danger @min-[56rem]:col-start-2')}>
            <PlayerName column={c} state={state} me={me} onOpenProfile={onOpenProfile} align="left" />
            {runes.length === 0
              ? <p className="text-caption text-muted">Sem runas nesta partida.</p>
              : <ul className="grid grid-cols-2 gap-x-3 gap-y-2">
                {runes.map(rune => (
                  <li key={rune.id} className="flex min-w-0 items-start gap-2 text-caption">
                    <GameIcon kind="perk" id={rune.id} size={24} />
                    <span className="min-w-0">
                      <span className="block font-semibold text-ink">{rune.name || `Runa ${rune.id}`}</span>
                      {rune.lines.map(line => <span key={line} className="block text-muted">{line}</span>)}
                    </span>
                  </li>
                ))}
              </ul>}
          </section>
        );
      })}
    </div>
  );
}
