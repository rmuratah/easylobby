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
      return <svg {...common}><path d="m12 3 8 3v5c0 5-3.4 8.5-8 10-4.6-1.5-8-5-8-10V6l8-3Z" /><path d="M9 8.5h6v4c0 2-1.2 3.5-3 4.3-1.8-.8-3-2.3-3-4.3v-4Z" /></svg>;
    case 'jungle':
      return <svg {...common}><path d="m8 4-4 6h3l-4 6h10l-4-6h3L8 4ZM17 6l-3 5h2l-3 5h8l-3-5h2l-3-5ZM8 16v4M17 16v4" /></svg>;
    case 'middle':
      return <svg {...common}><path d="m5 19 11-11 3 3L8 22l-3-3ZM14.5 9.5l-3-3 2-2 3 3-2 2ZM6 3v3M4.5 4.5h3M19 15v4M17 17h4" /></svg>;
    case 'bottom':
      return <svg {...common}><circle cx="12" cy="12" r="7" /><circle cx="12" cy="12" r="3" /><path d="M12 2v5M12 17v5M2 12h5M17 12h5" /></svg>;
    case 'utility':
      return <svg {...common}><path d="M12 14.5S8 12.3 8 9.5A2.5 2.5 0 0 1 12 7a2.5 2.5 0 0 1 4 2.5c0 2.8-4 5-4 5Z" /><path d="m3 12 3.5 5c.8 1.1 2.4 1.8 5.5 3M6.5 17 5 12.5c-.4-1.2 1.1-2 1.9-1l2.6 3M21 12l-3.5 5c-.8 1.1-2.4 1.8-5.5 3M17.5 17l1.5-4.5c.4-1.2-1.1-2-1.9-1l-2.6 3" /></svg>;
    case 'fill':
      return <svg {...common}><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" /></svg>;
    case 'unselected':
      return <svg {...common}><circle cx="12" cy="12" r="8" /><path d="M9.7 9a2.5 2.5 0 0 1 4.8 1c0 2-2.5 2-2.5 4M12 17h.01" /></svg>;
  }
}
