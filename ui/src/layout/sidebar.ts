import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

/** Duração da animação de fechar, a mesma de abrir (index.css). */
export const CLOSE_ANIMATION_MS = 160;

/**
 * Aba de amigos (#594): começa fechada, numa faixa fina à direita, e abre
 * por cima do conteúdo quando o mouse passa em cima, quando recebe o foco do
 * teclado ou pelo botão. Fecha na hora em que o mouse sai, com Esc ou com um
 * clique fora, deslizando de volta como abriu. Ao arrastar um amigo para o
 * lobby, ela sai da frente até o arraste acabar, sem desmontar quem está
 * sendo arrastado.
 */
export function useSidebar() {
  const [open, setOpen] = useState(false);
  const [closing, setClosing] = useState(false);
  const [dragging, setDragging] = useState(false);
  const regionRef = useRef<HTMLDivElement>(null);
  const closingTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  function stopClosing() {
    if (closingTimer.current !== null) clearTimeout(closingTimer.current);
    closingTimer.current = null;
    setClosing(false);
  }

  function show() {
    stopClosing();
    setOpen(true);
  }

  // Fecha já, mas a coluna continua na tela durante a animação de saída.
  function close() {
    if (!open) return;
    setOpen(false);
    setClosing(true);
    if (closingTimer.current !== null) clearTimeout(closingTimer.current);
    closingTimer.current = setTimeout(() => {
      closingTimer.current = null;
      setClosing(false);
    }, CLOSE_ANIMATION_MS);
  }

  useEffect(() => () => {
    if (closingTimer.current !== null) clearTimeout(closingTimer.current);
  }, []);

  // Clique ou toque fora da aba fecha (no toque não existe "mouse saiu").
  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!regionRef.current?.contains(event.target as Node)) close();
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  });

  const regionProps = {
    ref: regionRef,
    onMouseEnter: () => {
      if (!dragging) show();
    },
    onMouseLeave: close,
    onFocus: show,
    onBlur: (event: FocusEvent<HTMLDivElement>) => {
      const next = event.relatedTarget as Node | null;
      if (next && !event.currentTarget.contains(next)) close();
    },
    onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape" && open) {
        event.stopPropagation();
        close();
      }
    },
    // Esconder a origem no próprio dragstart cancela o arraste no Chromium; espera um ciclo.
    onDragStart: () => {
      setTimeout(() => setDragging(true), 0);
    },
    // A coluna já está fora da vista: fecha sem animação.
    onDragEnd: () => {
      stopClosing();
      setDragging(false);
      setOpen(false);
    },
  };

  function toggle() {
    if (open) close();
    else show();
  }

  return { open, closing, dragging, toggle, regionProps };
}
