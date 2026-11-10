import type { Wallet } from "../../bindings/Wallet";

const number = new Intl.NumberFormat("pt-BR");

type Currency = { key: keyof Wallet; label: string; short: string; color: string; shape: "gem" | "coin" };

// Cores próximas das do client oficial: azul, laranja e o dourado do RP.
const CURRENCIES: Currency[] = [
  { key: "blueEssence", label: "Essência Azul", short: "EA", color: "#4fa3e6", shape: "gem" },
  { key: "orangeEssence", label: "Essência Laranja", short: "EL", color: "#e8862f", shape: "gem" },
  { key: "rp", label: "RP", short: "RP", color: "#c9a253", shape: "coin" },
];

/**
 * Saldo da conta na barra do alto (#370): Essência Azul, Essência Laranja e
 * RP. Só leitura. Uma moeda que a LCU ainda não deu aparece como "—".
 */
export function WalletBalance({ wallet }: { wallet: Wallet }) {
  return (
    <ul data-tauri-drag-region aria-label="Saldo da conta" className="flex shrink-0 items-center gap-3 text-caption tabular-nums text-ink">
      {CURRENCIES.map(({ key, label, color, shape }) => {
        const value = wallet[key];
        const text = value === null ? "—" : number.format(value);
        return (
          <li key={key} data-tauri-drag-region title={`${label}: ${text}`} aria-label={`${label}: ${text}`}
            className="flex items-center gap-1">
            <svg aria-hidden="true" width="12" height="12" viewBox="0 0 12 12" fill={color}>
              {shape === "gem" ? <path d="M6 .5 11 6 6 11.5 1 6Z" /> : <circle cx="6" cy="6" r="5" />}
            </svg>
            <span data-tauri-drag-region>{text}</span>
          </li>
        );
      })}
    </ul>
  );
}
