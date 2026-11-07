import type { Position } from '../bindings/Position';

type Props = { position: Position; size?: number; className?: string };

/**
 * Um único mapa visual para cargos editáveis e cargos confirmados pelo agente.
 * Os traços são decorativos: o nome do cargo continua sempre visível ao lado.
 */
export function PositionIcon({ position, size = 18, className }: Props) {
  const common = {
    'aria-hidden': true,
    width: size,
    height: size,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 1.8,
    strokeLinecap: 'round' as const,
    strokeLinejoin: 'round' as const,
    className: `shrink-0 ${className ?? ''}`,
  };
  switch (position) {
    case 'top':
      return <svg {...common}><path d="M4 10V4h6M14 20h6v-6M5 19 19 5M8 16l8-8" /></svg>;
    case 'jungle':
      return <svg {...common}><path d="M12 20c0-6-2-10-6-14 4 1 6 4 7 8 1-5 3-8 6-10-1 6-2 12-7 16ZM9 14c-2-1-4-3-5-6" /></svg>;
    case 'middle':
      return <svg {...common}><rect x="4" y="4" width="16" height="16" rx="2" /><path d="m6 18 12-12M9 20l11-11M4 15 15-11" /></svg>;
    case 'bottom':
      return <svg {...common}><path d="M4 10V4h6M14 20h6v-6M5 19 19 5M8 16l8-8" transform="rotate(90 12 12)" /></svg>;
    case 'utility':
      return <svg {...common}><circle cx="12" cy="8" r="3" /><path d="M5 20c1-5 3-7 7-7s6 2 7 7M4 11h3M17 11h3" /></svg>;
    case 'fill':
      return <svg {...common}><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" /></svg>;
    case 'unselected':
      return <svg {...common}><circle cx="12" cy="12" r="8" /><path d="M9.7 9a2.5 2.5 0 0 1 4.8 1c0 2-2.5 2-2.5 4M12 17h.01" /></svg>;
  }
}
