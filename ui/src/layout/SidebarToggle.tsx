type Props = { open: boolean; onToggle: () => void; invitations: number; unread?: number };

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/**
 * Abre e recolhe a coluna de amigos; com ela recolhida, avisa os convites
 * pendentes e as mensagens não lidas, com um ponto de destaque (#132).
 */
export function SidebarToggle({ open, onToggle, invitations, unread = 0 }: Props) {
  const news = open ? [] : [
    invitations > 0 && count(invitations, "convite", "convites"),
    unread > 0 && count(unread, "mensagem", "mensagens"),
  ].filter(Boolean);
  return (
    <button type="button" aria-expanded={open} aria-controls="coluna-amigos" onClick={onToggle}
      className="sidebar-toggle inline-flex items-center gap-2">
      {news.length > 0 && <span aria-hidden="true" className="h-2 w-2 shrink-0 rounded-full bg-accent" />}
      {open ? "Recolher amigos" : "Mostrar amigos"}{news.length > 0 ? ` (${news.join(", ")})` : ""}
    </button>
  );
}
