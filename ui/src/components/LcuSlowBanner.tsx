import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { LaunchUxButton } from './LaunchUxButton';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };

/**
 * O LoL está demorando para responder (#580): o agente conectou, mas partes
 * do estado (fase, lobby, filas...) ficaram sem resposta e ele está tentando
 * de novo. Sem o aviso a tela fica vazia e parece que o app quebrou.
 */
export function LcuSlowBanner({ connection, send }: Props) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected' || !state.lcuSlow) return null;
  return (
    <section role="status" aria-live="polite" aria-label="LoL demorando para responder"
      className="col-span-full flex flex-col gap-3 rounded-card border border-warn bg-surface card p-4">
      <div className="flex flex-col gap-1">
        <h2 className="text-body font-semibold text-ink">O LoL está demorando para responder</h2>
        <p className="text-body text-ink">
          O easylobby está tentando de novo, e o lobby e os modos aparecem assim que o LoL responder.
          Isso costuma acontecer logo depois de abrir o LoL ou quando os servidores da Riot estão lentos.
          Se demorar, use o client oficial.
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <LaunchUxButton connection={connection} send={send} />
      </div>
    </section>
  );
}
