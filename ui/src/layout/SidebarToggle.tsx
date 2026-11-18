type Props = { open: boolean; onToggle: () => void; invitations: number; unread?: number };

const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/**
 * Abre e recolhe a aba de amigos (também abre com o mouse em cima, #594); com ela recolhida, avisa os convites
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
      className="sidebar-toggle relative inline-flex size-8 items-center justify-center p-0 text-muted transition-colors hover:text-ink">
      {news.length > 0 && <span aria-hidden="true" className="absolute right-0.5 top-0.5 h-1.5 w-1.5 rounded-full bg-accent" />}
      <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        {/* Fechada, a faixa mostra o ícone de amigos; aberta, a seta empurra a aba de volta para a direita. */}
        {open ? <path d="m9 18 6-6-6-6" /> : <>
          <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" /><circle cx="9" cy="7" r="4" />
          <path d="M22 21v-2a4 4 0 0 0-3-3.87" /><path d="M16 3.13a4 4 0 0 1 0 7.75" />
        </>}
      </svg>
    </button>
  );
}
