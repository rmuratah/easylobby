import type { AssetKind } from '../bindings/AssetKind';
import { AGENT_URL } from './client';

const ASSETS_BASE = (() => {
  const url = new URL(AGENT_URL);
  url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
  url.pathname = '/assets';
  return url.toString();
})();

/** Imagem servida pelo agente em `/assets/<tipo>/<id>`. */
export function assetUrl(kind: AssetKind, id: number): string {
  return `${ASSETS_BASE}/${kind}/${id}`;
}
