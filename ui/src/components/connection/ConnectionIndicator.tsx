import type { ConnectionView } from '../../screens/connection';
import { cx } from '../base/cx';

const DOT = { ready: 'bg-status-online', waiting: 'bg-status-away', stopped: 'bg-control' } as const;

/** Ponto e frase do estado da conexão, na barra de título. A frase vale mais que a cor. */
export function ConnectionIndicator({ view }: { view: ConnectionView }) {
  return (
    <p data-tauri-drag-region role="status" title={view.label} className="flex min-w-0 items-center gap-1.5 text-caption text-muted">
      <span aria-hidden="true" className={cx('h-2 w-2 shrink-0 rounded-full', DOT[view.tone])} />
      <span data-tauri-drag-region className="truncate">{view.label}</span>
    </p>
  );
}
