import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { useAgent } from "../agent/useAgent";
import { draftChampSelect } from "../test/champSelect";
import { testConversation, testInvitation } from "../test/events";
import { testState } from "../test/lobby";
import { makeState } from "../test/state";
import { SIDEBAR_STORAGE_KEY } from "./sidebar";

vi.mock("../agent/useAgent");

function snapshot(overrides: Parameters<typeof makeState>[0] = {}) {
  vi.mocked(useAgent).mockReturnValue({
    status: "connected", error: null, state: makeState({ ...testState, ...overrides }), send: vi.fn(() => new Promise<never>(() => {})),
  });
}

function windowWidth(narrow: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: narrow, media: query, addEventListener: () => {}, removeEventListener: () => {},
  }));
}

const champSelect = { gameflowPhase: "ChampSelect" as const, champSelect: draftChampSelect };
const column = () => document.getElementById("coluna-amigos")!;

beforeEach(() => {
  localStorage.clear();
  windowWidth(false);
});
afterEach(() => vi.unstubAllGlobals());

describe("coluna de amigos", () => {
  it("começa aberta, recolhe pelo botão e lembra a escolha", () => {
    snapshot();
    const { unmount } = render(<App />);
    expect(screen.getByRole("complementary", { name: "Amigos e conversas" })).toBe(column());
    const toggle = screen.getByRole("button", { name: "Recolher amigos" });
    expect(toggle.closest('.app-social-region')).not.toBeNull();
    expect(toggle.parentElement).toBe(column());
    expect(toggle.closest('header')).toBeNull();
    expect(toggle).not.toHaveClass('rounded-full', 'shadow-soft');
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(toggle).toHaveAttribute("aria-controls", column().id);
    fireEvent.click(toggle);
    expect(column()).not.toBeVisible();
    const collapsedToggle = screen.getByRole("button", { name: "Mostrar amigos" });
    expect(collapsedToggle).toHaveAttribute("aria-expanded", "false");
    expect(collapsedToggle.parentElement).not.toBe(column());
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBe("recolhida");
    unmount();
    render(<App />);
    expect(column()).not.toBeVisible();
  });

  it("avisa convites pendentes com a coluna recolhida", () => {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, "recolhida");
    snapshot({ receivedInvitations: [testInvitation] });
    render(<App />);
    expect(screen.getByRole("button", { name: "Mostrar amigos (1 convite)" })).toBeInTheDocument();
  });

  it("soma as mensagens não lidas ao aviso da coluna recolhida", () => {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, "recolhida");
    snapshot({ receivedInvitations: [testInvitation], conversations: [{ ...testConversation, unreadCount: 2 }] });
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos (1 convite, 2 mensagens)" }));
    expect(screen.getByRole("button", { name: "Recolher amigos" })).toBeInTheDocument();
  });

  it("na seleção de campeões com a janela estreita, recolhe sem mudar a escolha salva", () => {
    windowWidth(true);
    snapshot(champSelect);
    const { rerender } = render(<App />);
    expect(column()).not.toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos" }));
    expect(column()).toBeVisible();
    expect(localStorage.getItem(SIDEBAR_STORAGE_KEY)).toBeNull();
    snapshot();
    rerender(<App />);
    expect(column()).toBeVisible();
    snapshot(champSelect);
    rerender(<App />);
    expect(column()).not.toBeVisible();
  });

  it("na seleção de campeões com espaço, mantém a coluna aberta", () => {
    snapshot(champSelect);
    render(<App />);
    expect(column()).toBeVisible();
  });

  it("mantém a conversa aberta ao recolher e reabrir", () => {
    snapshot();
    render(<App />);
    const before = screen.getByRole("region", { name: "Amigos", hidden: true });
    fireEvent.click(screen.getByRole("button", { name: "Recolher amigos" }));
    fireEvent.click(screen.getByRole("button", { name: "Mostrar amigos" }));
    expect(screen.getByRole("region", { name: "Amigos" })).toBe(before);
  });
});
