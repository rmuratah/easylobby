import { useMemo, useState, type KeyboardEvent, type PointerEvent } from 'react';
import type { LpPoint } from '../../bindings/LpPoint';
import { LP_WINDOW_DAYS, daysAgo, lpSeries, pointLabel, scoreLabel } from '../../agent/lpChart';
import { cx } from '../base/cx';

const HEIGHT = 120;
/** Marcas do eixo de tempo, em dias antes de hoje. */
const TICKS = [30, 20, 10, 0];
const dateFormat = new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: '2-digit', year: 'numeric' });

/**
 * Gráfico de PDL dos últimos 30 dias de uma fila (#457), dentro do card de
 * elo: ganho no período, pico, linha com grade nas divisões e dica com data,
 * rank e PDL ao passar o mouse (ou com as setas do teclado).
 */
export function LpChart({ history, title, now }: { history: LpPoint[]; title: string; now: number }) {
  const series = useMemo(() => lpSeries(history, now), [history, now]);
  const [active, setActive] = useState<number | null>(null);
    if (!series) {
    return (
      <p className="mt-4 border-t border-line pt-3 text-caption text-muted">
        O gráfico de PDL começa na sua próxima ranqueada com o easylobby aberto.
      </p>
    );
  }
  const { points, start, end, min, max } = series;
  const x = (at: number) => ((at - start) / (end - start)) * 100;
  const y = (score: number) => (1 - (score - min) / (max - min)) * 100;
  const last = points.at(-1)!;
  // A linha vai até hoje: o rank continua o do último ponto.
  const path = [...points.map(p => [x(p.at), y(p.score)]), [100, y(last.score)]]
    .map(([px, py], i) => `${i === 0 ? 'M' : 'L'}${px.toFixed(2)} ${py.toFixed(2)}`).join(' ');
  const shown = active === null ? null : points[Math.min(active, points.length - 1)];

  function nearest(event: PointerEvent<HTMLDivElement>) {
    const box = event.currentTarget.getBoundingClientRect();
    const at = start + ((event.clientX - box.left) / box.width) * (end - start);
    let best = 0;
    points.forEach((p, i) => { if (Math.abs(p.at - at) < Math.abs(points[best].at - at)) best = i; });
    setActive(best);
  }
  function keys(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const current = active ?? points.length - 1;
    setActive(Math.max(0, Math.min(points.length - 1, current + (event.key === 'ArrowRight' ? 1 : -1))));
  }

  const gain = series.gain;
  return (
    <figure className="mt-4 border-t border-line pt-3" aria-label={`PDL dos últimos ${LP_WINDOW_DAYS} dias na ${title}`}>
      <figcaption className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-caption">
        <span className="flex items-center gap-2">
          <span className="font-semibold text-muted">Últimos {LP_WINDOW_DAYS} d</span>
          <span className={cx('font-semibold', gain > 0 ? 'text-status-online' : gain < 0 ? 'text-danger' : 'text-muted')}>
            <span aria-hidden="true">{gain > 0 ? '▲ ' : gain < 0 ? '▼ ' : ''}</span>
            {gain > 0 ? '+' : gain < 0 ? '−' : ''}{Math.abs(gain)} PDL
          </span>
        </span>
        <span className="rounded-full bg-raised px-2 py-0.5 font-semibold text-muted">
          Pico <span className="text-ink">{pointLabel(series.peak)}</span>
        </span>
      </figcaption>
      <div className="mt-2 flex">
        <div className="relative w-16 shrink-0 text-[0.6875rem] leading-none text-muted" style={{ height: HEIGHT }} aria-hidden="true">
          {series.grid.map(v => (
            <span key={v} className="absolute right-2 -translate-y-1/2 truncate" style={{ top: `${y(v)}%` }}>{scoreLabel(v)}</span>
          ))}
        </div>
        <div className="relative min-w-0 flex-1 cursor-crosshair touch-none rounded-sm outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
          style={{ height: HEIGHT }} tabIndex={0} role="img"
          aria-label={shown ? `${dateFormat.format(shown.point.at)}: ${pointLabel(shown.point)}` : 'Gráfico de PDL. Use as setas para ver cada ponto.'}
          onPointerMove={nearest} onPointerDown={nearest} onPointerLeave={() => setActive(null)}
          onKeyDown={keys} onBlur={() => setActive(null)}>
          <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-visible" aria-hidden="true">
            {series.grid.map(v => (
              <line key={v} x1={0} x2={100} y1={y(v)} y2={y(v)} className="stroke-line" strokeDasharray="3 4"
                vectorEffect="non-scaling-stroke" strokeWidth={1} />
            ))}
            {shown && (
              <line x1={x(shown.at)} x2={x(shown.at)} y1={0} y2={100} className="stroke-muted" strokeOpacity={0.5}
                vectorEffect="non-scaling-stroke" strokeWidth={1} />
            )}
            <path d={path} fill="none" className="stroke-accent" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round"
              vectorEffect="non-scaling-stroke" />
          </svg>
          {shown && (
            <>
              <span aria-hidden="true" className="pointer-events-none absolute size-2.5 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-surface bg-accent"
                style={{ left: `${x(shown.at)}%`, top: `${y(shown.score)}%` }} />
              <div data-testid="lp-tooltip" aria-hidden="true"
                className={cx('pointer-events-none absolute top-0 z-10 w-max rounded-control-sm border border-line bg-surface px-2.5 py-1.5 text-caption shadow-soft',
                  x(shown.at) > 55 ? '-translate-x-[calc(100%+10px)]' : 'translate-x-[10px]')}
                style={{ left: `${x(shown.at)}%` }}>
                <p className="font-semibold text-ink">{pointLabel(shown.point)}</p>
                <p className="text-muted">{dateFormat.format(shown.point.at)} · {daysAgo(shown.point.at, end)}</p>
                <p className="text-muted">{shown.point.wins}V {shown.point.losses}D</p>
              </div>
            </>
          )}
        </div>
      </div>
      <div className="ml-16 mt-1.5 flex justify-between text-[0.6875rem] text-muted" aria-hidden="true">
        {TICKS.map(d => <span key={d}>{d === 0 ? 'hoje' : `${d} d`}</span>)}
      </div>
      <table className="sr-only">
        <caption>PDL por data</caption>
        <tbody>
          {points.map(p => (
            <tr key={`${p.at}-${p.score}`}><td>{dateFormat.format(p.point.at)}</td><td>{pointLabel(p.point)}</td></tr>
          ))}
        </tbody>
      </table>
    </figure>
  );
}
