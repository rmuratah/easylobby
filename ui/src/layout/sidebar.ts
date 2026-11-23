import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent } from "react";

/** Duração da animação de fechar, a mesma de abrir: a faixa encolhe de volta (index.css). */
export const CLOSE_ANIMATION_MS = 180;

/** Aba fixada (#643): a escolha fica salva entre sessões. */
export const PINNED_STORAGE_KEY = "easylobby.amigos.fixa";

function readPinned(): boolean {
  try {
    return localStorage.getItem(PINNED_STORAGE_KEY) === "sim";
  } catch {
    return false;
  }
}

function storePinned(pinned: boolean) {
  try {
    if (pinned) localStorage.setItem(PINNED_STORAGE_KEY, "sim");
    else localStorage.removeItem(PINNED_STORAGE_KEY);
  } catch {
    // Sem armazenamento, a escolha vale só até fechar o app.
  }
}

/**
 * Aba de amigos (#594): começa fechada, numa faixa fina à direita, e abre
 * por cima do conteúdo quando o mouse passa em cima, quando recebe o foco do
 * teclado ou pelo botão. Fecha na hora em que o mouse sai, com Esc ou com um
 * clique fora, deslizando de volta como abriu. Ao arrastar um amigo para o
 * lobby, ela sai da frente até o arraste acabar, sem desmontar quem está
 * sendo arrastado.
 *
 * Fixada (#643), fica sempre aberta ao lado do conteúdo, empurrando-o em vez
 * de ficar por cima, e nada além do próprio botão de fixar a fecha.
 */
export function useSidebar() {
  const [pinned, setPinned] = useState(readPinned);
  const [hoverOpen, setOpen] = useState(false);
  const open = pinned || hoverOpen;
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
    if (pinned || !hoverOpen) return;
    animateClose();
  }

  function animateClose() {
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
    if (!open || pinned) return;
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
      if (event.key === "Escape" && open && !pinned) {
        event.stopPropagation();
        close();
      }
    },
    // Esconder a origem no próprio dragstart cancela o arraste no Chromium; espera um ciclo.
    // Fixada, a aba não cobre o lobby e fica onde está.
    onDragStart: () => {
      if (!pinned) setTimeout(() => setDragging(true), 0);
    },
    // A coluna já está fora da vista: fecha sem animação.
    onDragEnd: () => {
      stopClosing();
      setDragging(false);
      setOpen(false);
    },
  };

  // Recolher a aba fixada também a desafixa.
  function toggle() {
    if (pinned) {
      setPinned(false);
      storePinned(false);
      animateClose();
    } else if (open) close();
    else show();
  }

  // Desafixar deixa a aba aberta como num mouse em cima: fecha quando o mouse sair.
  function togglePinned() {
    const next = !pinned;
    stopClosing();
    setPinned(next);
    setOpen(!next);
    storePinned(next);
  }

  return { open, closing, dragging, pinned, toggle, togglePinned, regionProps };
}
