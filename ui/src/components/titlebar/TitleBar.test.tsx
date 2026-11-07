import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot } from "../../agent/client";
import { makeState } from "../../test/state";
import { TitleBar } from "./TitleBar";
import type { WindowControls } from "./windowControls";

const connected: ConnectionSnapshot = { status: "connected", error: null, state: makeState() };

function fakeWindow(maximized = false) {
  let resized: (() => void) | null = null;
  const unlisten = vi.fn();
  const controls: WindowControls = {
    minimize: vi.fn(async () => {}),
    toggleMaximize: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
    isMaximized: vi.fn(async () => maximized),
    onResized: vi.fn(async (listener: () => void) => { resized = listener; return unlisten; }),
  };
  const resize = async (value: boolean) => {
    maximized = value;
    await act(async () => { resized?.(); });
  };
  return { controls, resize, unlisten };
}

function renderBar(controls: WindowControls | null) {
  return render(
    <TitleBar connection={connected} send={vi.fn()} controls={controls} />,
  );
}

describe("TitleBar", () => {
  it("arrasta pelo espaço livre, mas não pelos botões", () => {
    const { container } = renderBar(fakeWindow().controls);
    const header = container.querySelector("header")!;
    expect(header).toHaveAttribute("data-tauri-drag-region");
    for (const button of screen.getAllByRole("button")) expect(button).not.toHaveAttribute("data-tauri-drag-region");
  });

  it("minimiza, maximiza e fecha a janela", async () => {
    const { controls } = fakeWindow();
    renderBar(controls);
    fireEvent.click(screen.getByRole("button", { name: "Minimizar" }));
    fireEvent.click(await screen.findByRole("button", { name: "Maximizar" }));
    fireEvent.click(screen.getByRole("button", { name: "Fechar (continua na bandeja)" }));
    expect(controls.minimize).toHaveBeenCalledOnce();
    expect(controls.toggleMaximize).toHaveBeenCalledOnce();
    expect(controls.close).toHaveBeenCalledOnce();
  });

  it("troca Maximizar por Restaurar quando a janela muda de tamanho e para de ouvir ao sair", async () => {
    const { controls, resize, unlisten } = fakeWindow(true);
    const { unmount } = renderBar(controls);
    expect(await screen.findByRole("button", { name: "Restaurar" })).toBeInTheDocument();
    await resize(false);
    expect(screen.getByRole("button", { name: "Maximizar" })).toBeInTheDocument();
    unmount();
    await act(async () => {});
    expect(unlisten).toHaveBeenCalledOnce();
  });

  it("no navegador, sem Tauri, esconde os botões da janela", () => {
    renderBar(null);
    expect(screen.queryByRole("button", { name: "Minimizar" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Abrir client oficial" })).toBeInTheDocument();
  });

  it("mantém a recuperação do client em formato compacto e sem o controle social", () => {
    renderBar(null);
    expect(screen.getByRole("button", { name: "Abrir client oficial" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Recolher amigos" })).not.toBeInTheDocument();
  });
});
