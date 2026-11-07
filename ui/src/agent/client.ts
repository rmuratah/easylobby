import type { AgentState } from '../bindings/AgentState';
import type { ClientMessage } from '../bindings/ClientMessage';
import type { Command } from '../bindings/Command';
import type { CommandResult } from '../bindings/CommandResult';
import type { ServerMessage } from '../bindings/ServerMessage';
import { parseMessage } from './protocol';

export const AGENT_URL = 'ws://127.0.0.1:47800/ws';
export type ConnectionSnapshot = {
  status: 'connecting' | 'connected' | 'reconnecting' | 'incompatible';
  state: AgentState | null;
  error: string | null;
};
type Pending = { resolve: (result: CommandResult) => void; reject: (error: Error) => void; timer: ReturnType<typeof setTimeout> };

type StateEvent = Exclude<ServerMessage, { type: 'hello' | 'response' | 'invalidMessage' }>;

// Aplica um evento do agente ao estado. O switch é exaustivo: quando o
// agente ganhar um evento novo, o TypeScript acusa aqui até ele ser tratado.
export function applyEvent(state: AgentState, message: StateEvent): AgentState {
  switch (message.type) {
    case 'lcu': return { ...state, lcu: message.status,
      readyCheck: message.status.state === 'connected' && state.lcu.state === 'connected' &&
        message.status.session === state.lcu.session ? state.readyCheck : null,
      champSelectChat: message.status.state === 'connected' && state.lcu.state === 'connected' &&
        message.status.session === state.lcu.session ? state.champSelectChat : null };
    case 'summoner': return { ...state, summoner: message.summoner };
    case 'gameflowPhase': return { ...state, gameflowPhase: message.phase,
      champSelectChat: message.phase === 'ChampSelect' ? state.champSelectChat : null };
    case 'readyCheck': return { ...state, readyCheck: message.readyCheck };
    case 'lobby': return { ...state, lobby: message.lobby };
    case 'matchmaking': return { ...state, matchmaking: message.matchmaking };
    case 'queues': return { ...state, queues: message.queues };
    case 'champSelect': return { ...state, champSelect: message.champSelect,
      champSelectChat: message.champSelect && (!state.champSelect || state.champSelect.id === message.champSelect.id)
        ? state.champSelectChat : null };
    case 'champSelectChat': return { ...state, champSelectChat:
      state.lcu.state === 'connected' && state.gameflowPhase === 'ChampSelect' && state.champSelect
        ? message.champSelectChat : null };
    case 'receivedInvitations': return { ...state, receivedInvitations: message.invitations };
    case 'conversation': {
      const { conversation } = message;
      const known = state.conversations.some(c => c.id === conversation.id);
      const conversations = known
        ? state.conversations.map(c => c.id === conversation.id ? conversation : c)
        : [...state.conversations, conversation];
      return { ...state, conversations };
    }
    case 'conversationClosed': return { ...state, conversations: state.conversations.filter(c => c.id !== message.id) };
    case 'runePages': return { ...state, runePages: message.runePages };
    case 'runeCatalog': return { ...state, runeCatalog: message.runeCatalog };
    case 'summonerSpellCatalog': return { ...state, summonerSpellCatalog: message.summonerSpellCatalog };
    case 'championCatalog': return { ...state, championCatalog: message.championCatalog };
    case 'champSelectChampions': return { ...state, champSelectChampions: message.champSelectChampions };
    case 'friends': return { ...state, friends: message.friends };
    case 'friendGroups': return { ...state, friendGroups: message.friendGroups };
    default: return message satisfies never;
  }
}

export class AgentClient {
  private snapshot: ConnectionSnapshot = { status: 'connecting', state: null, error: null };
  private listeners = new Set<() => void>();
  private socket: WebSocket | null = null;
  private retryTimer?: ReturnType<typeof setTimeout>;
  private helloTimer?: ReturnType<typeof setTimeout>;
  private active = false;
  private attempts = 0;
  private nextId = 0;
  private pending = new Map<number, Pending>();

  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };
  private publish(snapshot: ConnectionSnapshot) {
    this.snapshot = snapshot;
    this.listeners.forEach(listener => listener());
  }
  start = () => {
    if (this.active) return;
    this.active = true;
    this.attempts = 0;
    this.publish({ status: 'connecting', state: null, error: null });
    this.connect();
  };
  stop = () => {
    this.active = false;
    this.release();
    clearTimeout(this.retryTimer);
    this.rejectPending();
  };
  private release() {
    clearTimeout(this.helloTimer);
    const socket = this.socket;
    this.socket = null;
    if (socket) {
      socket.onmessage = socket.onclose = socket.onerror = null;
      socket.close();
    }
  }
  private rejectPending() {
    this.pending.forEach(({ reject, timer }) => {
      clearTimeout(timer);
      reject(new Error('Conexão com o agente encerrada.'));
    });
    this.pending.clear();
  }
  private reconnect(error: string) {
    if (!this.active) return;
    this.release();
    this.rejectPending();
    this.publish({ status: 'reconnecting', state: null, error });
    clearTimeout(this.retryTimer);
    const delay = Math.min(1000 * 2 ** Math.min(this.attempts++, 3), 8000);
    this.retryTimer = setTimeout(() => this.connect(), delay);
  }
  private connect() {
    if (!this.active) return;
    try {
      const socket = new WebSocket(AGENT_URL);
      this.socket = socket;
      this.helloTimer = setTimeout(() => this.reconnect('O agente não respondeu a tempo.'), 10000);
      socket.onerror = () => { if (this.socket === socket) this.reconnect('Não foi possível conectar ao agente.'); };
      socket.onclose = () => { if (this.socket === socket) this.reconnect('Conexão com o agente interrompida.'); };
      socket.onmessage = event => {
        if (this.socket !== socket || !this.active) return;
        try {
          if (typeof event.data !== 'string') throw new Error('Mensagem inválida do agente.');
          const message = parseMessage(event.data);
          if (!message) return;
          if (message.type === 'hello') {
            clearTimeout(this.helloTimer);
            if (message.protocol !== 1) {
              this.release();
              this.rejectPending();
              this.publish({ status: 'incompatible', state: null, error: 'Atualize o aplicativo e o agente para versões compatíveis.' });
              return;
            }
            this.attempts = 0;
            this.publish({ status: 'connected', state: message.state, error: null });
            return;
          }
          const state = this.snapshot.state;
          if (!state) throw new Error('O agente não enviou o estado inicial.');
          if (message.type === 'response') {
            const pending = this.pending.get(message.id);
            if (pending) {
              clearTimeout(pending.timer);
              this.pending.delete(message.id);
              pending.resolve(message.result);
            }
            return;
          }
          if (message.type === 'invalidMessage') {
            this.publish({ ...this.snapshot, error: message.message });
            return;
          }
          this.publish({ ...this.snapshot, state: applyEvent(state, message) });
        } catch {
          this.reconnect('Recebemos uma mensagem inválida do agente.');
        }
      };
    } catch {
      this.reconnect('Não foi possível conectar ao agente.');
    }
  }
  send = (command: Command): Promise<CommandResult> => {
    const socket = this.socket;
    if (this.snapshot.status !== 'connected' || !socket || socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error('O agente ainda não está conectado.'));
    }
    const id = this.nextId = (this.nextId % 0xffffffff) + 1;
    const message: ClientMessage = { id, command };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error('O agente não respondeu ao comando.'));
      }, 10000);
      this.pending.set(id, { resolve, reject, timer });
      try { socket.send(JSON.stringify(message)); }
      catch {
        this.pending.delete(id);
        clearTimeout(timer);
        reject(new Error('Não foi possível enviar o comando.'));
      }
    });
  };
}
