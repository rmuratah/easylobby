import type { Position } from '../bindings/Position';

type Props = { position: Position; size?: number; className?: string };

/**
 * Desenhos oficiais das rotas do client do LoL
 * (`rcp-fe-lol-static-assets/svg/position-*.svg`), guardados aqui para não
 * depender de caminhos do client que mudam com o patch (#482). Como no
 * client, cada rota tem uma parte principal e uma parte apagada (metade da
 * opacidade). A cor vem de `currentColor`, para seguir o tema e os estados
 * de cada tela. Os desenhos são decorativos: o nome do cargo continua sempre
 * visível ao lado.
 */
const DRAWINGS: Record<Exclude<Position, 'unselected'>, { main: string; dim?: string }> = {
  top: {
    main: 'M4 4l.003 24.045L9 23V9h14l5.045-4.997L4 4Z',
    dim: 'M21,14H14v7h7V14Zm5-3V26L11.014,26l-4,4H30V7.016Z',
  },
  jungle: {
    main: 'M25,3c-2.128,3.3-5.147,6.851-6.966,11.469A42.373,42.373,0,0,1,20,20a27.7,27.7,0,0,1,1-3C21,12.023,22.856,8.277,25,3ZM13,20c-1.488-4.487-4.76-6.966-9-9,3.868,3.136,4.422,7.52,5,12l3.743,3.312C14.215,27.917,16.527,30.451,17,31c4.555-9.445-3.366-20.8-8-28C11.67,9.573,13.717,13.342,13,20Zm8,5a15.271,15.271,0,0,1,0,2l4-4c0.578-4.48,1.132-8.864,5-12C24.712,13.537,22.134,18.854,21,25Z',
  },
  middle: {
    main: 'M25 4 4 25v5h5L30 9V4h-5Z',
    dim: 'M30,12.968l-4.008,4L26,26H17l-4,4H30ZM16.979,8L21,4H4V20.977L8,17,8,8h8.981Z',
  },
  bottom: {
    main: 'M29.997 5.955 25 11v14H11l-5.045 4.997L30 30l-.003-24.045Z',
    dim: 'M13,20h7V13H13v7ZM4,4V26.984l3.955-4L8,8,22.986,8l4-4H4Z',
  },
  utility: {
    main: 'M26,13c3.535,0,8-4,8-4H23l-3,3,2,7,5-2-3-4h2ZM22,5L20.827,3H13.062L12,5l5,6Zm-5,9-1-1L13,28l4,3,4-3L18,13ZM11,9H0s4.465,4,8,4h2L7,17l5,2,2-7Z',
  },
  // "Preencher": as quinas do topo e do atirador acesas, com um losango no
  // meio, no mesmo traço dos outros.
  fill: {
    main: 'M4 4l.003 24.045L9 23V9h14l5.045-4.997L4 4ZM29.997 5.955 25 11v14H11l-5.045 4.997L30 30l-.003-24.045ZM17 11l6 6-6 6-6-6 6-6Z',
  },
};

export function PositionIcon({ position, size = 18, className }: Props) {
  const common = {
    'aria-hidden': true,
    width: size,
    height: size,
    className: `shrink-0 ${className ?? ''}`,
  };
  if (position === 'unselected') {
    return (
      <svg {...common} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="8" /><path d="M9.7 9a2.5 2.5 0 0 1 4.8 1c0 2-2.5 2-2.5 4M12 17h.01" />
      </svg>
    );
  }
  const { main, dim } = DRAWINGS[position];
  return (
    <svg {...common} viewBox="0 0 34 34" fill="currentColor">
      {dim && <path opacity={0.5} fillRule="evenodd" d={dim} />}
      <path fillRule="evenodd" d={main} />
    </svg>
  );
}
