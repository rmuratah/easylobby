import { useCallback, useState } from "react";

export const NAVIGATION_STORAGE_KEY = "easylobby.navegacao";

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(NAVIGATION_STORAGE_KEY) === "recolhida";
  } catch {
    return false;
  }
}

function storeCollapsed(collapsed: boolean) {
  try {
    localStorage.setItem(NAVIGATION_STORAGE_KEY, collapsed ? "recolhida" : "expandida");
  } catch {
    // Sem armazenamento a escolha vale só até fechar o app.
  }
}

/** Navegação lateral expandida (ícone e rótulo) ou recolhida (só ícone); a escolha fica salva. */
export function useNavigationCollapsed() {
  const [collapsed, setCollapsed] = useState(readCollapsed);
  const toggle = useCallback(() => {
    setCollapsed(current => {
      storeCollapsed(!current);
      return !current;
    });
  }, []);
  return { collapsed, toggle };
}
