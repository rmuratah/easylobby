import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { Card } from './base';
import { RunePageEditor } from './RuneEditor';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

const heading = <h2 className="text-title font-semibold tracking-[-0.015em]">Páginas de runas</h2>;

/**
 * Páginas de runas da seleção de campeões (#26), em todos os modos com
 * runas. Usa o mesmo editor do pop-up do Jogo Dinâmico (#420): escolher uma
 * página a aplica, e o cabeçalho renomeia, cria e exclui páginas. Só mostra
 * o estado confirmado pelo agente; o rascunho do editor é a única coisa
 * local, e some quando a conexão com o LoL muda.
 */
export function RunePagesPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  let body;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    body = <p className="text-body text-muted">Aguarde a reconexão. As páginas serão confirmadas novamente pelo agente.</p>;
  } else if (!state.runePages) {
    body = <p className="text-body text-muted">{state.summoner ? 'Carregando páginas de runas…' : 'Aguarde o login no League of Legends.'}</p>;
  } else if (!state.runeCatalog) {
    body = <p className="text-body text-muted">Carregando as árvores de runas…</p>;
  } else {
    body = (
      <RunePageEditor key={`${lcu.session}:${state.summoner?.puuid ?? ''}`}
        catalog={state.runeCatalog} runePages={state.runePages} send={send} />
    );
  }
  return (
    <Card as="section" aria-label="Páginas de runas" className="flex w-full max-w-[56rem] flex-col gap-4 p-5 text-ink">
      {heading}
      {body}
    </Card>
  );
}
