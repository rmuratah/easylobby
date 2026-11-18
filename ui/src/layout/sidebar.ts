import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

/** Tempo para o mouse sair da aba e voltar sem ela fechar no meio do caminho. */
export const CLOSE_DELAY_MS = 250;

/**
 * Aba de amigos (#594): começa fechada, numa faixa fina à direita, e abre
 * por cima do conteúdo quando o mouse passa em cima, quando recebe o foco do
 * teclado ou pelo botão. Fecha quando o mouse sai, com Esc ou com um clique
 * fora. Ao arrastar um amigo para o lobby, ela sai da frente até o arraste
 * acabar, sem desmontar quem está sendo arrastado.
 */
export function useSidebar() {
  const [open, setOpen] = useState(false);
  const [dragging, setDragging] = useState(false);
  const regionRef = useRef<HTMLDivElement>(null);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  function cancelClose() {
    if (closeTimer.current !== null) clearTimeout(closeTimer.current);
    closeTimer.current = null;
  }

  useEffect(() => cancelClose, []);

  // Clique ou toque fora da aba fecha (no toque não existe "mouse saiu").
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!regionRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  const regionProps = {
    ref: regionRef,
    onMouseEnter: () => {
      cancelClose();
      if (!dragging) setOpen(true);
    },
    onMouseLeave: () => {
      cancelClose();
      closeTimer.current = setTimeout(() => {
        closeTimer.current = null;
        setOpen(false);
      }, CLOSE_DELAY_MS);
    },
    onFocus: () => {
      cancelClose();
      setOpen(true);
    },
    onBlur: (event: FocusEvent<HTMLDivElement>) => {
      const next = event.relatedTarget as Node | null;
      if (next && !event.currentTarget.contains(next)) setOpen(false);
    },
    onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape" && open) {
        event.stopPropagation();
        setOpen(false);
      }
    },
    // Esconder a origem no próprio dragstart cancela o arraste no Chromium; espera um ciclo.
    onDragStart: () => {
      setTimeout(() => setDragging(true), 0);
    },
    onDragEnd: () => {
      cancelClose();
      setDragging(false);
      setOpen(false);
    },
  };

  function toggle() {
    cancelClose();
    setOpen(!open);
  }

  return { open, dragging, toggle, regionProps };
}
