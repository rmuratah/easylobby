import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { useAgent } from "../agent/useAgent";
import { testConversation, testInvitation } from "../test/events";
import { testState } from "../test/lobby";
import { makeState } from "../test/state";
import { CLOSE_ANIMATION_MS, PINNED_STORAGE_KEY } from "./sidebar";

vi.mock("../agent/useAgent");

function snapshot(overrides: Parameters<typeof makeState>[0] = {}) {
  vi.mocked(useAgent).mockReturnValue({
    status: "connected", error: null, state: makeState({ ...testState, ...overrides }), send: vi.fn(() => new Promise<never>(() => {})),
  });
}

const column = () => document.getElementById("coluna-amigos")!;
const region = () => column().closest<HTMLElement>(".app-social-region")!;
/** Fecha na hora (sem a classe de aberta) e some depois da animação de saída. */
function expectClosing() {
  expect(region()).not.toHaveClass("is-open");
  expect(region()).toHaveClass("is-closing");
  act(() => vi.advanceTimersByTime(CLOSE_ANIMATION_MS));
  expect(region()).not.toHaveClass("is-closing");
  expect(column()).not.toBeVisible();
}

afterEach(() => { vi.useRealTimers(); localStorage.clear(); });

describe("aba de amigos (#594)", () => {
  it("começa fechada, numa faixa fina com o botão", () => {
    localStorage.setItem("easylobby.amigos", "aberta");
    snapshot();
    render(<App />);
    expect(column()).not.toBeVisible();
    const toggle = screen.getByRole("button", { name: "Mostrar amigos" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    expect(toggle).toHaveAttribute("aria-controls", column().id);
    expect(toggle.closest(".app-social-rail")).not.toBeNull();
    expect(toggle.closest("header")).toBeNull();
  });

  it("abre por cima do conteúdo com o mouse em cima e fecha quando o mouse sai", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    const body = document.querySelector(".app-body")!;
    fireEvent.mouseEnter(region());
    expect(column()).toBeVisible();
    expect(screen.getByRole("complementary", { name: "Amigos e convites" })).toBe(column());
    expect(region()).toHaveClass("is-open");
    // Sobrepõe: a área principal não ganha nem perde classe de layout.
    expect(body.className).toBe("app-body");
    fireEvent.mouseLeave(region());
    // Sem atraso: começa a fechar na hora, ainda visível durante a animação.
    expect(column()).toBeVisible();
    expectClosing();
  });

  it("o mouse voltando no meio da animação de fechar reabre", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    fireEvent.mouseEnter(region());
    fireEvent.mouseLeave(region());
    act(() => vi.advanceTimersByTime(CLOSE_ANIMATION_MS / 2));
    fireEvent.mouseEnter(region());
    expect(region()).toHaveClass("is-open");
    expect(region()).not.toHaveClass("is-closing");
    act(() => vi.advanceTimersByTime(CLOSE_ANIMATION_MS * 2));
    expect(column()).toBeVisible();
  });

  it("abre pelo botão e fecha com Esc ou clique fora", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos" }));
    expect(column()).toBeVisible();
    fireEvent.keyDown(column(), { key: "Escape" });
    expectClosing();
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos" }));
    expect(column()).toBeVisible();
    fireEvent.pointerDown(document.querySelector(".app-content")!);
    expectClosing();
  });

  it("o botão dentro da aba aberta recolhe", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    fireEvent.mouseEnter(region());
    const close = screen.getByRole("button", { name: "Recolher amigos" });
    expect(close.closest("aside")).toBe(column());
    expect(close).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(close);
    expectClosing();
  });

  it("avisa convites pendentes com a aba fechada", () => {
    snapshot({ receivedInvitations: [testInvitation] });
    render(<App />);
    expect(screen.getByRole("button", { name: "Mostrar amigos (1 convite)" })).toBeInTheDocument();
  });

  it("soma as mensagens não lidas ao aviso da aba fechada", () => {
    snapshot({ receivedInvitations: [testInvitation], conversations: [{ ...testConversation, unreadCount: 2 }] });
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos (1 convite, 2 mensagens)" }));
    expect(screen.getByRole("button", { name: "Recolher amigos" })).toBeInTheDocument();
  });

  it("ao arrastar um amigo, sai da frente sem desmontar e fecha no fim do arraste", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    fireEvent.mouseEnter(region());
    const friends = screen.getByRole("region", { name: "Amigos" });
    fireEvent.dragStart(column());
    act(() => vi.advanceTimersByTime(0));
    expect(region()).toHaveClass("is-dragging");
    expect(column()).not.toHaveAttribute("hidden");
    fireEvent.dragEnd(column());
    expect(region()).not.toHaveClass("is-dragging");
    expect(column()).not.toBeVisible();
    fireEvent.mouseEnter(region());
    expect(screen.getByRole("region", { name: "Amigos" })).toBe(friends);
  });
});

describe("aba de amigos fixada (#643)", () => {
  it("fixar deixa a aba aberta, empurrando o conteúdo, e salva a escolha", () => {
    vi.useFakeTimers();
    snapshot();
    render(<App />);
    fireEvent.mouseEnter(region());
    const pin = screen.getByRole("button", { name: "Fixar amigos" });
    expect(pin).toHaveAttribute("aria-pressed", "false");
    fireEvent.click(pin);
    expect(region()).toHaveClass("is-open", "is-pinned");
    expect(screen.getByRole("button", { name: "Desafixar amigos" })).toHaveAttribute("aria-pressed", "true");
    expect(localStorage.getItem(PINNED_STORAGE_KEY)).toBe("sim");
    // Mouse saindo, Esc e clique fora não fecham.
    fireEvent.mouseLeave(region());
    fireEvent.keyDown(column(), { key: "Escape" });
    fireEvent.pointerDown(document.querySelector(".app-content")!);
    act(() => vi.advanceTimersByTime(CLOSE_ANIMATION_MS * 2));
    expect(region()).toHaveClass("is-open");
    expect(region()).not.toHaveClass("is-closing");
    expect(column()).toBeVisible();
  });

  it("abre já fixada quando a escolha está salva", () => {
    localStorage.setItem(PINNED_STORAGE_KEY, "sim");
    snapshot();
    render(<App />);
    expect(column()).toBeVisible();
    expect(region()).toHaveClass("is-pinned");
  });

  it("desafixar volta a abrir com o mouse em cima", () => {
    vi.useFakeTimers();
    localStorage.setItem(PINNED_STORAGE_KEY, "sim");
    snapshot();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Desafixar amigos" }));
    expect(localStorage.getItem(PINNED_STORAGE_KEY)).toBeNull();
    expect(region()).not.toHaveClass("is-pinned");
    // Continua aberta enquanto o mouse está em cima e fecha quando ele sai.
    expect(column()).toBeVisible();
    fireEvent.mouseLeave(region());
    expectClosing();
  });

  it("recolher a aba fixada também desafixa", () => {
    vi.useFakeTimers();
    localStorage.setItem(PINNED_STORAGE_KEY, "sim");
    snapshot();
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Recolher amigos" }));
    expect(localStorage.getItem(PINNED_STORAGE_KEY)).toBeNull();
    expectClosing();
    expect(region()).not.toHaveClass("is-pinned");
  });

  it("fixada, arrastar um amigo não esconde a aba", () => {
    vi.useFakeTimers();
    localStorage.setItem(PINNED_STORAGE_KEY, "sim");
    snapshot();
    render(<App />);
    fireEvent.dragStart(column());
    act(() => vi.advanceTimersByTime(0));
    expect(region()).not.toHaveClass("is-dragging");
    fireEvent.dragEnd(column());
    expect(column()).toBeVisible();
  });
});
