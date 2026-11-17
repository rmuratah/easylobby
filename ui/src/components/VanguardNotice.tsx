import type { ConnectionSnapshot } from '../agent/client';
import type { Vanguard } from '../bindings/Vanguard';

/** O Vanguard desconectado (#579), ou `null` com ele conectado, sem resposta da LCU ou sem o LoL. */
function disconnectedVanguard(connection: ConnectionSnapshot): Vanguard | null {
  if (connection.status !== 'connected' || connection.state?.lcu.state !== 'connected') return null;
  const vanguard = connection.state.vanguard;
  return vanguard && !vanguard.connected ? vanguard : null;
}

/**
 * Aviso de Vanguard desconectado (#579). Sem ele a partida não abre: o LoL
 * encerra a seleção no fim e conta esquiva. O client oficial avisa e desliga
 * o botão Jogar; com ele fechado, o aviso é este. Não some com um clique,
 * porque entrar na fila assim (ou pelo líder do grupo) sempre vira esquiva.
 */
export function VanguardBanner({ connection }: { connection: ConnectionSnapshot }) {
  const vanguard = disconnectedVanguard(connection);
  if (!vanguard) return null;
  return (
    <section role="alert" aria-label="Vanguard desconectado"
      className="col-span-full flex flex-col gap-2 rounded-card border border-warn bg-surface card p-4">
      <h2 className="text-body font-semibold text-ink">O Vanguard não está conectado</h2>
      <p className="text-body text-ink">
        Sem o Vanguard a partida não abre: o LoL encerra a seleção de campeões no fim e conta como esquiva para
        você, mesmo se o líder do grupo entrar na fila. Não entre na fila até este aviso sumir.
      </p>
      <p className="text-body text-muted">
        Feche e abra o LoL de novo. Se continuar, reinicie o computador e, se ainda assim não resolver,
        reinstale o Vanguard pelo Riot Client.
      </p>
      <p className="text-caption text-muted">
        Estado informado pelo LoL: <code>{vanguard.state}</code>
        {vanguard.status !== null && <> (código {vanguard.status})</>}
      </p>
    </section>
  );
}
