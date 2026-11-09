import type { ReactNode } from 'react';

/** Traços simples, na cor do texto, para os botões e selos do início e do lobby. */
function Icon({ size = 16, children }: { size?: number; children: ReactNode }) {
  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" className="shrink-0">
      {children}
    </svg>
  );
}

export const CloseIcon = ({ size }: { size?: number }) => <Icon size={size}><path d="M18 6 6 18M6 6l12 12" /></Icon>;
export const PlusIcon = ({ size }: { size?: number }) => <Icon size={size}><path d="M12 5v14M5 12h14" /></Icon>;
export const CrownIcon = ({ size }: { size?: number }) => (
  <Icon size={size}><path d="m2 7 4.5 4L12 4l5.5 7L22 7l-2 12H4L2 7Z" /></Icon>
);
/** Duas setas opostas: trocar de vaga no lobby da Arena (#224). */
export const SwapIcon = ({ size }: { size?: number }) => (
  <Icon size={size}><path d="M7 4 3 8l4 4M3 8h14M17 12l4 4-4 4M21 16H7" /></Icon>
);
