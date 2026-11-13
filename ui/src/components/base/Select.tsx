import {
  useCallback, useEffect, useId, useLayoutEffect, useRef, useState,
  type CSSProperties, type KeyboardEvent, type ReactNode, type Ref,
} from "react";
import { createPortal } from "react-dom";
import { cx } from "./cx";

export type SelectOption<T extends string | number> = {
  value: T;
  label: string;
  /** Ícone à esquerda do texto, na lista e no botão quando a opção está escolhida. */
  icon?: ReactNode;
  /** Texto menor embaixo do rótulo, só na lista (por exemplo, "recomendada"). */
  hint?: ReactNode;
  disabled?: boolean;
};

export type SelectSize = "sm" | "md";

type Props<T extends string | number> = {
  value: T | null;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  id?: string;
  /** Nome acessível quando não há um `<label htmlFor>` nem `aria-labelledby`. */
  "aria-label"?: string;
  "aria-labelledby"?: string;
  /** Texto do botão quando nenhuma opção está escolhida. */
  placeholder?: string;
  disabled?: boolean;
  size?: SelectSize;
  /** Classes do botão (largura, peso da fonte...). */
  className?: string;
  ref?: Ref<HTMLButtonElement>;
};

const heights: Record<SelectSize, string> = {
  sm: "h-9 px-2.5 text-caption",
  md: "h-10 px-3 text-body",
};

/** Altura máxima da lista aberta, em px (18rem). */
const LIST_MAX = 288;
const GAP = 6;

type Placement = { top?: number; bottom?: number; left: number; minWidth: number; maxHeight: number };

/**
 * Lista de seleção com o visual do app (#479), no lugar do `<select>` nativo.
 *
 * Segue o padrão "select-only combobox" da WAI-ARIA: o foco fica no botão, e a
 * opção ativa vem por `aria-activedescendant`. Setas, Home/End, PageUp/PageDown,
 * Enter/Espaço, Esc, Tab e busca digitando. A lista abre num portal em posição
 * fixa, para não ser cortada por painéis com rolagem nem por pop-ups.
 */
export function Select<T extends string | number>({
  value, options, onChange, id, placeholder = "Escolha", disabled = false, size = "md", className, ref,
  "aria-label": ariaLabel, "aria-labelledby": ariaLabelledBy,
}: Props<T>) {
  const autoId = useId();
  const buttonId = id ?? autoId;
  const listId = `${buttonId}-lista`;
  const optionId = (index: number) => `${buttonId}-opcao-${index}`;
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const [expanded, setOpen] = useState(false);
  // Desabilitado com a lista aberta (por exemplo, enquanto um comando roda): a lista some.
  const open = expanded && !disabled;
  const [active, setActive] = useState(-1);
  const [placement, setPlacement] = useState<Placement | null>(null);
  const typed = useRef({ text: "", at: 0 });

  const selectedIndex = options.findIndex(option => option.value === value);
  const selected = selectedIndex >= 0 ? options[selectedIndex] : undefined;

  const setButton = useCallback((node: HTMLButtonElement | null) => {
    buttonRef.current = node;
    if (typeof ref === "function") ref(node);
    else if (ref) ref.current = node;
  }, [ref]);

  function enabledFrom(start: number, step: 1 | -1): number {
    for (let index = start; index >= 0 && index < options.length; index += step) {
      if (!options[index].disabled) return index;
    }
    return -1;
  }
  const first = () => enabledFrom(0, 1);
  const last = () => enabledFrom(options.length - 1, -1);

  function show(index: number) {
    if (disabled) return;
    setActive(index >= 0 ? index : selectedIndex >= 0 && !options[selectedIndex].disabled ? selectedIndex : first());
    setOpen(true);
  }

  function close() {
    setOpen(false);
    setActive(-1);
  }

  function choose(index: number) {
    const option = options[index];
    close();
    if (!option || option.disabled) return;
    if (option.value !== value) onChange(option.value);
  }

  /** Busca digitando: acumula as letras por meio segundo e pula para a próxima opção que começa com elas. */
  function typeahead(key: string): number {
    const now = Date.now();
    const text = now - typed.current.at > 500 ? key : typed.current.text + key;
    typed.current = { text, at: now };
    const from = open && active >= 0 ? active : selectedIndex;
    const ordered = options.map((_, index) => (from + (text.length === 1 ? 1 : 0) + index) % options.length);
    const match = ordered.find(index => !options[index].disabled
      && options[index].label.toLocaleLowerCase("pt-BR").startsWith(text.toLocaleLowerCase("pt-BR")));
    return match ?? -1;
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (disabled) return;
    const { key } = event;
    if (!open) {
      if (key === "ArrowDown" || key === "ArrowUp" || key === "Enter" || key === " ") { event.preventDefault(); show(-1); }
      else if (key === "Home") { event.preventDefault(); show(first()); }
      else if (key === "End") { event.preventDefault(); show(last()); }
      else if (key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
        const match = typeahead(key);
        if (match >= 0) { event.preventDefault(); show(match); }
      }
      return;
    }
    const step = (from: number, delta: 1 | -1) => {
      const next = enabledFrom(from + delta, delta);
      return next >= 0 ? next : from;
    };
    switch (key) {
      case "ArrowDown":
        event.preventDefault();
        if (event.altKey) return;
        setActive(current => step(current, 1));
        return;
      case "ArrowUp":
        event.preventDefault();
        if (event.altKey) { choose(active); return; }
        setActive(current => step(current, -1));
        return;
      case "Home": event.preventDefault(); setActive(first()); return;
      case "End": event.preventDefault(); setActive(last()); return;
      case "PageDown": event.preventDefault(); setActive(current => enabledFrom(Math.min(current + 10, options.length - 1), -1)); return;
      case "PageUp": event.preventDefault(); setActive(current => enabledFrom(Math.max(current - 10, 0), 1)); return;
      case "Enter":
      case " ":
        event.preventDefault();
        choose(active);
        return;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        close();
        return;
      case "Tab":
        choose(active);
        return;
      default:
        if (key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
          const match = typeahead(key);
          if (match >= 0) { event.preventDefault(); setActive(match); }
        }
    }
  }

  // Posição da lista: embaixo do botão, ou em cima quando falta espaço. Acompanha rolagem e redimensionamento.
  useLayoutEffect(() => {
    if (!open) return;
    function place() {
      const button = buttonRef.current;
      if (!button) return;
      const rect = button.getBoundingClientRect();
      const below = window.innerHeight - rect.bottom - GAP - 8;
      const above = rect.top - GAP - 8;
      const wanted = Math.min(LIST_MAX, listRef.current?.scrollHeight ?? LIST_MAX);
      const up = below < wanted && above > below;
      setPlacement({
        left: rect.left,
        minWidth: rect.width,
        maxHeight: Math.max(96, Math.min(LIST_MAX, up ? above : below)),
        ...(up ? { bottom: window.innerHeight - rect.top + GAP } : { top: rect.bottom + GAP }),
      });
    }
    place();
    window.addEventListener("resize", place);
    const onScroll = (event: Event) => { if (event.target !== listRef.current) place(); };
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [open]);

  // Clique fora fecha sem trocar a opção.
  useEffect(() => {
    if (!open) return;
    function onPointerDown(event: PointerEvent) {
      const target = event.target as Node;
      if (buttonRef.current?.contains(target) || listRef.current?.contains(target)) return;
      close();
    }
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open]);

  // Mantém a opção ativa visível na lista com rolagem.
  useEffect(() => {
    if (!open || active < 0) return;
    document.getElementById(optionId(active))?.scrollIntoView?.({ block: "nearest" });
  });

  const style: CSSProperties | undefined = placement
    ? { position: "fixed", ...placement }
    : { position: "fixed", visibility: "hidden" };

  return (
    <>
      <button
        ref={setButton}
        id={buttonId}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open && active >= 0 ? optionId(active) : undefined}
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        data-value={value ?? ""}
        disabled={disabled}
        onClick={() => (open ? close() : show(-1))}
        onKeyDown={onKeyDown}
        onBlur={event => {
          if (open && !listRef.current?.contains(event.relatedTarget as Node | null)) close();
        }}
        className={cx(
          "group inline-flex min-w-0 items-center gap-2 rounded-control border bg-raised text-left text-ink transition-colors",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent",
          "enabled:hover:border-accent disabled:cursor-not-allowed disabled:text-muted",
          open ? "border-accent" : "border-control",
          heights[size],
          className,
        )}
      >
        {selected?.icon && <span aria-hidden="true" className="flex shrink-0">{selected.icon}</span>}
        <span className={cx("min-w-0 flex-1 truncate", !selected && "text-muted")}>{selected?.label ?? placeholder}</span>
        <Chevron open={open} />
      </button>
      {open && createPortal(
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          tabIndex={-1}
          aria-labelledby={ariaLabelledBy ?? buttonId}
          style={style}
          // O foco fica no botão: clicar na lista não pode tirar o foco dele.
          onMouseDown={event => event.preventDefault()}
          className="z-[60] flex flex-col gap-0.5 overflow-y-auto rounded-control-lg border border-line bg-surface p-1 text-ink shadow-lift outline-none animate-select-in"
        >
          {options.map((option, index) => {
            const isSelected = index === selectedIndex;
            return (
              <li
                key={String(option.value)}
                id={optionId(index)}
                role="option"
                data-value={option.value}
                aria-selected={isSelected}
                aria-disabled={option.disabled || undefined}
                onPointerMove={() => { if (!option.disabled && active !== index) setActive(index); }}
                onClick={() => { if (!option.disabled) { choose(index); buttonRef.current?.focus(); } }}
                className={cx(
                  "flex min-h-9 items-center gap-2 rounded-control-sm px-2.5 py-1.5 text-body",
                  option.disabled ? "cursor-not-allowed text-muted opacity-60" : "cursor-pointer",
                  isSelected ? "bg-accent-soft font-semibold text-accent-ink" : index === active && !option.disabled && "bg-raised",
                  isSelected && index === active && "ring-1 ring-inset ring-accent/40",
                )}
              >
                {option.icon && <span aria-hidden="true" className="flex shrink-0">{option.icon}</span>}
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="truncate">{option.label}</span>
                  {option.hint && <span className="truncate text-caption font-normal text-muted">{option.hint}</span>}
                </span>
                <Check visible={isSelected} />
              </li>
            );
          })}
        </ul>,
        document.body,
      )}
    </>
  );
}

function Chevron({ open }: { open: boolean }) {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.25"
      strokeLinecap="round" strokeLinejoin="round"
      className={cx("shrink-0 text-muted transition-transform duration-150 group-enabled:group-hover:text-accent-ink", open && "rotate-180 text-accent-ink")}>
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

function Check({ visible }: { visible: boolean }) {
  return (
    <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"
      strokeLinecap="round" strokeLinejoin="round" className={cx("shrink-0 text-accent-ink", !visible && "invisible")}>
      <path d="M5 12.5l4.5 4.5L19 7.5" />
    </svg>
  );
}
