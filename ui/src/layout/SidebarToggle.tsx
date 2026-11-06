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
  const label = `${open ? "Recolher" : "Mostrar"} amigos${news.length > 0 ? ` (${news.join(", ")})` : ""}`;
  return (
    <button type="button" aria-label={label} title={open ? "Recolher amigos" : "Mostrar amigos"}
      aria-expanded={open} aria-controls="coluna-amigos" onClick={onToggle}
      className="sidebar-toggle relative inline-flex size-10 items-center justify-center rounded-full p-0 text-muted shadow-soft transition-colors hover:text-ink">
      {news.length > 0 && <span aria-hidden="true" className="absolute right-1.5 top-1.5 h-2 w-2 rounded-full bg-accent" />}
      <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d={open ? "m15 18-6-6 6-6" : "m9 18 6-6-6-6"} />
      </svg>
    </button>
  );
}
