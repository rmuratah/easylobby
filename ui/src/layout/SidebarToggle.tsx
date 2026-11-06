type Props = { open: boolean; onToggle: () => void; invitations: number };

/** Abre e recolhe a coluna de amigos; com ela recolhida, avisa os convites pendentes. */
export function SidebarToggle({ open, onToggle, invitations }: Props) {
  const pending = !open && invitations > 0
    ? ` (${invitations} ${invitations === 1 ? "convite" : "convites"})` : "";
  return (
    <button type="button" aria-expanded={open} aria-controls="coluna-amigos" onClick={onToggle} className="sidebar-toggle">
      {open ? "Recolher amigos" : "Mostrar amigos"}{pending}
    </button>
  );
}
