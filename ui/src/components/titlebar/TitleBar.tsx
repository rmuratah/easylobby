import { useEffect, useState, type ReactNode } from "react";
import type { AgentClient, ConnectionSnapshot } from "../../agent/client";
import { LaunchUxButton } from "../LaunchUxButton";
import { ConnectionIndicator } from "../connection/ConnectionIndicator";
import { connectionView } from "../../screens/connection";
import { tauriWindowControls, type WindowControls } from "./windowControls";
import { SidebarToggle } from "../../layout/SidebarToggle";

type Props = {
  connection: ConnectionSnapshot;
  send: AgentClient["send"];
  sidebarOpen: boolean;
  onSidebarToggle: () => void;
  invitations: number;
  unread: number;
  /** Injetável nos testes; por padrão, a janela do Tauri (ou nada, no navegador). */
  controls?: WindowControls | null;
};

const defaultControls = tauriWindowControls();

/**
 * Controles compactos da moldura (#126, #160). O espaço vazio com
 * `data-tauri-drag-region` arrasta a janela; nenhum botão faz parte da região.
 */
export function TitleBar({ connection, send, sidebarOpen, onSidebarToggle, invitations, unread, controls = defaultControls }: Props) {
  return (
    <header
      data-tauri-drag-region
      className="app-toolbar select-none text-ink"
    >
      <ConnectionIndicator view={connectionView(connection)} />
      <div data-tauri-drag-region className="h-full min-w-4 flex-1" />
      <LaunchUxButton connection={connection} send={send} compact />
      <SidebarToggle open={sidebarOpen} onToggle={onSidebarToggle} invitations={invitations} unread={unread} />
      {controls ? <WindowButtons controls={controls} /> : <div className="w-1" />}
    </header>
  );
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
