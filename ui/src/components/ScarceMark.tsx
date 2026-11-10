import { cx } from './base/cx';

/** Marca das posições prioritárias (as que faltam na fila), como o losango azul do client oficial. */
export function ScarceMark({ className }: { className?: string }) {
  return <span aria-hidden="true" className={cx('size-2.5 rotate-45 rounded-[2px] bg-accent ring-2 ring-surface', className)} />;
}
