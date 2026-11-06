import type { AssetKind } from '../bindings/AssetKind';
import { AGENT_URL } from './client';

/** Endereço HTTP do agente, o mesmo host e porta do `/ws`. */
const AGENT_HTTP = new URL(AGENT_URL.replace(/^ws/, 'http')).origin;

/** URL de uma imagem servida pelo agente em `/assets/<tipo>/<id>`. */
export function assetUrl(kind: AssetKind, id: number): string {
  return `${AGENT_HTTP}/assets/${kind}/${id}`;
}
