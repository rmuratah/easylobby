import { useEffect, useState } from "react";

export const SIDEBAR_STORAGE_KEY = "easylobby.amigos";
/** Abaixo dessa largura, a seleção de campeões precisa da janela inteira. */
export const NARROW_QUERY = "(max-width: 1279px)";

type Stored = "aberta" | "recolhida";

function readStored(): Stored | null {
  try {
    const value = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    return value === "aberta" || value === "recolhida" ? value : null;
  } catch {
    return null;
  }
}

function store(value: Stored) {
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, value);
  } catch {
    // Sem armazenamento a escolha vale só até fechar o app.
  }
}

function useNarrow(): boolean {
  const [narrow, setNarrow] = useState(() => typeof matchMedia === "function" && matchMedia(NARROW_QUERY).matches);
  useEffect(() => {
    if (typeof matchMedia !== "function") return;
    const query = matchMedia(NARROW_QUERY);
    const update = () => setNarrow(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return narrow;
}

/**
 * Coluna de amigos (#127): aberta ou recolhida conforme a última escolha do
 * jogador, que fica salva. Na seleção de campeões com a janela estreita ela
 * começa recolhida para dar espaço; abrir ali vale só até a seleção acabar.
 */
export function useSidebar(champSelect: boolean) {
  const [stored, setStored] = useState<Stored>(() => readStored() ?? "aberta");
  const [openedInChampSelect, setOpenedInChampSelect] = useState(false);
  const [wasChampSelect, setWasChampSelect] = useState(champSelect);
  const narrow = useNarrow();
  if (wasChampSelect !== champSelect) {
    setWasChampSelect(champSelect);
    setOpenedInChampSelect(false);
  }
  const autoCollapse = champSelect && narrow;
  const open = autoCollapse ? stored === "aberta" && openedInChampSelect : stored === "aberta";

  function toggle() {
    if (autoCollapse && stored === "aberta") {
      setOpenedInChampSelect(!openedInChampSelect);
      return;
    }
    const next: Stored = open ? "recolhida" : "aberta";
    setStored(next);
    store(next);
    if (autoCollapse) setOpenedInChampSelect(true);
  }

  return { open, toggle };
}
