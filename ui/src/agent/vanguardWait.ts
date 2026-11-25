import { useEffect, useSyncExternalStore } from 'react';
import type { Vanguard } from '../bindings/Vanguard';
import type { ConnectionSnapshot } from './client';

/**
 * Código do Vanguard para o LoL aberto muitas vezes em pouco tempo (erro VAN
 * 216, #668). A Riot manda esperar pelo menos 30 minutos sem abrir o jogo, e
 * abrir antes disso reinicia a contagem
 * (https://support.riotgames.com/pt-br/riot/performance/error-van-216).
 *
 * Nos logs, o Vanguard recusa a conexão 1 s depois de conectar e o LoL fecha
 * sozinho de 24 a 45 s depois. Reabrir o LoL nesse meio-tempo só piora.
 */
const VANGUARD_LAUNCH_LIMIT = 216;

/** Quanto tempo esperar sem abrir o LoL depois do VAN 216. */
export const VANGUARD_WAIT_MS = 30 * 60_000;

/** Quando o app viu o último VAN 216 (ms desde 1970). */
const STORAGE_KEY = 'easylobby.vanguard.recusa';

/** A recusa, quando o armazenamento não funciona: vale só até fechar o app. */
let fallback: number | null = null;
const listeners = new Set<() => void>();

function readStored(): number | null {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored ? Number(stored) : null;
  } catch {
    return fallback;
  }
}

/** A hora da última recusa, ou `null` sem recusa nos últimos 30 minutos. */
function refusedAt(): number | null {
  const at = readStored();
  const now = Date.now();
  return at !== null && Number.isFinite(at) && at <= now && now < at + VANGUARD_WAIT_MS ? at : null;
}

function store(at: number | null) {
  fallback = at;
  try {
    if (at === null) localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, String(at));
  } catch {
    // Sem armazenamento, fica só o `fallback`.
  }
  listeners.forEach(listener => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** Se o Vanguard recusou a conexão porque o LoL foi aberto vezes demais (VAN 216). */
export function refusedForLaunches(vanguard: Vanguard | null): boolean {
  return !!vanguard && !vanguard.connected && vanguard.status === VANGUARD_LAUNCH_LIMIT;
}

/** Hora local de `ms`, arredondada para o minuto de cima (`21:53`): esperar até ela nunca é pouco. */
export function clockTime(ms: number): string {
  const date = new Date(Math.ceil(ms / 60_000) * 60_000);
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

/**
 * Fim da espera do VAN 216 (ms desde 1970) enquanto ela dura, ou `null` (#668).
 *
 * O LoL fecha sozinho logo depois da recusa, e com ele some o estado do
 * Vanguard; por isso a hora fica salva e vale também para a tela de LoL
 * fechado e depois de abrir o app de novo. Cada recusa nova recomeça os 30
 * minutos, e o Vanguard conectado apaga a espera.
 */
export function useVanguardWait(connection: ConnectionSnapshot): number | null {
  const vanguard = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.vanguard : null;
  const refused = refusedForLaunches(vanguard);
  const connected = vanguard?.connected === true;
  const at = useSyncExternalStore(subscribe, refusedAt);
  useEffect(() => {
    if (refused) store(Date.now());
  }, [refused]);
  useEffect(() => {
    if (connected) store(null);
  }, [connected]);
  // Some sozinha quando os 30 minutos acabam.
  useEffect(() => {
    if (at === null) return;
    const timer = setTimeout(() => store(null), at + VANGUARD_WAIT_MS - Date.now());
    return () => clearTimeout(timer);
  }, [at]);
  return at === null ? null : at + VANGUARD_WAIT_MS;
}
