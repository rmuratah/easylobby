import type { ServerMessage } from '../bindings/ServerMessage';
import type { AgentState } from '../bindings/AgentState';
import type { LcuStatus } from '../bindings/LcuStatus';
import type { Summoner } from '../bindings/Summoner';
import type { ErrorCode } from '../bindings/ErrorCode';

// Exige atualizar a validação quando o contrato gerado ganhar ou remover códigos.
const errorCodes = {
  notConnected: true,
  lcu: true,
  launchFailed: true,
  invalidCommand: true,
} satisfies Record<ErrorCode, true>;

function errorCode(v: unknown): v is ErrorCode {
  return typeof v === 'string' && Object.hasOwn(errorCodes, v);
}

const object = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const uint = (v: unknown): v is number => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
const nullableString = (v: unknown) => v === null || typeof v === 'string';
function summoner(v: unknown): v is Summoner | null {
  return v === null || (object(v) && ['gameName', 'tagLine', 'displayName', 'puuid'].every(k => typeof v[k] === 'string') &&
    ['summonerLevel', 'profileIconId', 'summonerId'].every(k => uint(v[k])));
}
function status(v: unknown): v is LcuStatus {
  if (!object(v)) return false;
  switch (v.state) {
    case 'starting': return true;
    case 'connected': return uint(v.session);
    case 'disconnected': return typeof v.reason === 'string';
    case 'retrying': return uint(v.attempt) && uint(v.retryInMs) && typeof v.clientNotRunning === 'boolean' && typeof v.error === 'string';
    default: return false;
  }
}
function state(v: unknown): v is AgentState {
  return object(v) && status(v.lcu) && summoner(v.summoner) && nullableString(v.gameflowPhase);
}

// Os tipos vêm do Rust; esta validação protege a fronteira JSON em runtime.
// Mensagens novas são ignoradas para permitir extensões compatíveis do protocolo.
export function parseMessage(text: string): ServerMessage | null {
  const v: unknown = JSON.parse(text);
  if (!object(v) || typeof v.type !== 'string') throw new Error('Mensagem inválida do agente.');
  let valid: boolean;
  switch (v.type) {
    case 'hello': valid = uint(v.protocol) && typeof v.agentVersion === 'string' && state(v.state); break;
    case 'lcu': valid = status(v.status); break;
    case 'summoner': valid = summoner(v.summoner); break;
    case 'gameflowPhase': valid = nullableString(v.phase); break;
    case 'response': valid = uint(v.id) && object(v.result) && (v.result.status === 'ok' ||
      (v.result.status === 'error' && errorCode(v.result.code) && typeof v.result.message === 'string')); break;
    case 'invalidMessage': valid = typeof v.message === 'string'; break;
    default: return null;
  }
  if (!valid) throw new Error('Mensagem inválida do agente.');
  return v as ServerMessage;
}
