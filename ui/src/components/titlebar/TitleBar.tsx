import { useEffect, useState, type ReactNode } from "react";
import type { AgentClient, ConnectionSnapshot } from "../../agent/client";
import { Segmented } from "../base";
import { LaunchUxButton } from "../LaunchUxButton";
import { ConnectionIndicator } from "../connection/ConnectionIndicator";
import { connectionView } from "../../screens/connection";
import type { Theme } from "../../theme/theme";
import { tauriWindowControls, type WindowControls } from "./windowControls";

const THEME_OPTIONS: { value: Theme; label: ReactNode; ariaLabel: string }[] = [
  { value: "claro", label: <SunIcon />, ariaLabel: "Tema claro" },
  { value: "escuro", label: <MoonIcon />, ariaLabel: "Tema escuro" },
];

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient["send"];
  theme: Theme;
  onThemeChange: (preference: Theme) => void;
  /** Injetável nos testes; por padrão, a janela do Tauri (ou nada, no navegador). */
  controls?: WindowControls | null;
};

const defaultControls = tauriWindowControls();

/**
 * Barra de título do app, no lugar da moldura do Windows (#126). As áreas com
 * `data-tauri-drag-region` arrastam a janela e maximizam no duplo clique; o
 * Tauri cuida disso sozinho, inclusive do encaixe nas bordas.
 */
export function TitleBar({ connection, send, theme, onThemeChange, controls = defaultControls }: Props) {
  return (
    <header
      data-tauri-drag-region
      className="flex h-12 shrink-0 select-none items-center gap-3 border-b border-line bg-surface pl-4 text-ink"
    >
      <h1 data-tauri-drag-region className="text-section font-semibold tracking-[-0.01em]">easylobby</h1>
      <ConnectionIndicator view={connectionView(connection)} />
      <div data-tauri-drag-region className="h-full min-w-4 flex-1" />
      <LaunchUxButton connection={connection} send={send} />
      <Segmented label="Tema" value={theme} onChange={onThemeChange} options={THEME_OPTIONS} className="shrink-0" />
      {controls ? <WindowButtons controls={controls} /> : <div className="w-1" />}
    </header>
  );
}

function SunIcon() {
  return <svg aria-hidden="true" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="4" /><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" /></svg>;
}

function MoonIcon() {
  return <svg aria-hidden="true" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M20 15.5A8.5 8.5 0 0 1 8.5 4 8.5 8.5 0 1 0 20 15.5Z" /></svg>;
}

function WindowButtons({ controls }: { controls: WindowControls }) {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let active = true;
    const refresh = () => { controls.isMaximized().then((value) => { if (active) setMaximized(value); }, () => {}); };
    refresh();
    const stop = controls.onResized(refresh);
    return () => {
      active = false;
      void stop.then((unlisten) => unlisten(), () => {});
    };
  }, [controls]);
  return (
    <div className="flex h-full items-stretch">
      <WindowButton label="Minimizar" onClick={controls.minimize}>
        <path d="M2 6h8" />
      </WindowButton>
      <WindowButton label={maximized ? "Restaurar" : "Maximizar"} onClick={controls.toggleMaximize}>
        {maximized ? <><rect x="2" y="4" width="6" height="6" /><path d="M4 4V2h6v6H8" /></> : <rect x="2" y="2" width="8" height="8" />}
      </WindowButton>
      <WindowButton label="Fechar (continua na bandeja)" onClick={controls.close} danger>
        <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" />
      </WindowButton>
    </div>
  );
}

type WindowButtonProps = { label: string; onClick: () => Promise<void>; danger?: boolean; children: ReactNode };

function WindowButton({ label, onClick, danger = false, children }: WindowButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={() => { onClick().catch((error: unknown) => console.error(`erro na janela (${label}):`, error)); }}
      className={
        "flex w-12 items-center justify-center text-muted hover:text-ink focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent " +
        (danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-raised")
      }
    >
      <svg aria-hidden="true" width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1">
        {children}
      </svg>
    </button>
  );
}
