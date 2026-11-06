import { cx } from "./cx";

export type PresenceStatus = "online" | "emPartida" | "ausente" | "offline";

const STATUS: Record<PresenceStatus, { label: string; dot: string }> = {
  online: { label: "Online", dot: "bg-status-online" },
  emPartida: { label: "Em partida", dot: "bg-status-game" },
  ausente: { label: "Ausente", dot: "bg-status-away" },
  offline: { label: "Offline", dot: "bg-control" },
};

type Props = {
  status: PresenceStatus;
  /** Texto ao lado do ponto; por padrão, o nome do status. Cor nunca é a única pista. */
  text?: string;
  className?: string;
};

export function StatusBadge({ status, text, className }: Props) {
  const { label, dot } = STATUS[status];
  return (
    <span className={cx("inline-flex items-center gap-1.5 text-caption text-muted", className)}>
      <span aria-hidden="true" className={cx("h-2 w-2 shrink-0 rounded-full", dot)} />
      {text ?? label}
    </span>
  );
}

/** Só o ponto, para cantos de avatar; o status precisa estar escrito em outro lugar ou no rótulo. */
export function StatusDot({ status, className }: { status: PresenceStatus; className?: string }) {
  return (
    <span
      role="img"
      aria-label={STATUS[status].label}
      className={cx("inline-block h-2.5 w-2.5 rounded-full ring-2 ring-surface", STATUS[status].dot, className)}
    />
  );
}
