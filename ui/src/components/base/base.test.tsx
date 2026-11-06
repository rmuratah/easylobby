import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  Avatar,
  Button,
  EmptyState,
  ErrorMessage,
  initials,
  SearchField,
  Segmented,
  Spinner,
  StatusBadge,
  StatusDot,
  TextField,
} from ".";

describe("Button", () => {
  it("é do tipo button por padrão, para não enviar formulários sem querer", () => {
    render(<Button>Entrar na fila</Button>);
    expect(screen.getByRole("button", { name: "Entrar na fila" })).toHaveAttribute("type", "button");
  });

  it("bloqueia cliques e avisa o carregamento sem perder o texto", () => {
    const onClick = vi.fn();
    render(<Button variant="primary" loading onClick={onClick}>Aceitar</Button>);
    const button = screen.getByRole("button", { name: "Aceitar" });
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-busy", "true");
    fireEvent.click(button);
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe("TextField", () => {
  it("liga o rótulo ao campo, mesmo escondido", () => {
    render(<SearchField label="Buscar campeão" placeholder="Digite um nome" />);
    expect(screen.getByRole("searchbox", { name: "Buscar campeão" })).toBeInTheDocument();
  });

  it("marca o erro e o descreve para leitores de tela", () => {
    render(<TextField label="Nome da página" error="Use até 25 letras." />);
    const input = screen.getByRole("textbox", { name: "Nome da página" });
    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(input).toHaveAccessibleDescription("Use até 25 letras.");
  });
});

describe("Segmented", () => {
  it("marca a opção atual e avisa a escolhida", () => {
    const onChange = vi.fn();
    render(
      <Segmented
        label="Posição principal"
        value="meio"
        onChange={onChange}
        options={[{ value: "topo", label: "Topo" }, { value: "meio", label: "Meio" }]}
      />,
    );
    expect(screen.getByRole("group", { name: "Posição principal" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Meio" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Topo" }));
    expect(onChange).toHaveBeenCalledWith("topo");
  });
});

describe("Avatar", () => {
  it("monta iniciais a partir do nome", () => {
    expect(initials("Rafa")).toBe("RA");
    expect(initials("Miss Fortune")).toBe("MF");
    expect(initials("  ")).toBe("?");
  });

  it("usa as iniciais quando a imagem falha", () => {
    const { container } = render(<Avatar name="Ahri" src="/assets/champion/103" labelled />);
    const img = container.querySelector("img")!;
    fireEvent.error(img);
    expect(screen.getByRole("img", { name: "Ahri" })).toHaveTextContent("AH");
  });
});

describe("StatusBadge", () => {
  it("escreve o status além da cor", () => {
    render(<><StatusBadge status="emPartida" /><StatusDot status="ausente" /></>);
    expect(screen.getByText("Em partida")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Ausente" })).toBeInTheDocument();
  });
});

describe("Spinner e mensagens", () => {
  it("anuncia o carregamento", () => {
    render(<Spinner />);
    expect(screen.getByRole("status", { name: "Carregando" })).toBeInTheDocument();
  });

  it("anuncia erros e mostra a ação de saída", () => {
    render(<ErrorMessage title="Não foi possível entrar na fila" description="Confira se o LoL está aberto." action={<Button>Tentar de novo</Button>} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Não foi possível entrar na fila");
    expect(screen.getByRole("button", { name: "Tentar de novo" })).toBeInTheDocument();
  });

  it("mostra o próximo passo no estado vazio", () => {
    render(<EmptyState title="Nenhum amigo online" description="Convide alguém pelo nome de invocador." />);
    expect(screen.getByRole("heading", { name: "Nenhum amigo online" })).toBeInTheDocument();
  });
});
