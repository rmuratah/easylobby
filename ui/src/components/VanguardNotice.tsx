import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { clockTime, refusedForLaunches } from '../agent/vanguardWait';
import type { Vanguard } from '../bindings/Vanguard';
import { Button } from './base';
import { cx } from './base/cx';

type Feedback = { status: 'idle' | 'pending' | 'error'; message: string };

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
 *
 * No VAN 216 (#668), reabrir o LoL reinicia a espera do Vanguard: o aviso
 * manda esperar até `waitUntil` e não oferece o botão.
 */
export function VanguardBanner({ connection, send, waitUntil = null }: {
  connection: ConnectionSnapshot; send: AgentClient['send'];
  /** Fim da espera do VAN 216 (ms desde 1970), de `useVanguardWait`. */
  waitUntil?: number | null;
}) {
  const vanguard = disconnectedVanguard(connection);
  if (!vanguard) return null;
  return (
    <section role="alert" aria-label="Vanguard desconectado"
      className="col-span-full flex flex-col gap-2 rounded-card border border-warn bg-surface card p-4">
      {refusedForLaunches(vanguard) ? <>
        <h2 className="text-body font-semibold text-ink">
          {waitUntil === null ? 'Espere 30 minutos para abrir o LoL de novo' : `Espere até as ${clockTime(waitUntil)} para abrir o LoL de novo`}
        </h2>
        <p className="text-body text-ink">
          O LoL foi aberto muitas vezes em pouco tempo, e o Vanguard recusou a conexão (erro VAN 216). Sem o
          Vanguard a partida não abre, e o LoL fecha sozinho daqui a pouco.
        </p>
        <p className="text-body text-muted">
          Abrir o LoL antes desse horário faz a espera de 30 minutos começar de novo, por isso reabrir agora não
          resolve.
        </p>
      </> : <>
        <h2 className="text-body font-semibold text-ink">O Vanguard não está conectado</h2>
        <p className="text-body text-ink">
          Sem o Vanguard a partida não abre: o LoL encerra a seleção de campeões no fim e conta como esquiva para
          você, mesmo se o líder do grupo entrar na fila. Não entre na fila até este aviso sumir.
        </p>
        <p className="text-body text-muted">
          Reabrir o LoL costuma resolver. Se continuar, reinicie o computador e, se ainda assim não resolver,
          reinstale o Vanguard pelo Riot Client.
        </p>
        <RestartLeagueButton send={send} />
      </>}
      <p className="text-caption text-muted">
        Estado informado pelo LoL: <code>{vanguard.state}</code>
        {vanguard.status !== null && <> (código {vanguard.status})</>}
      </p>
    </section>
  );
}

/**
 * A espera do VAN 216 na tela de LoL fechado (#668): o LoL fecha sozinho
 * logo depois da recusa, e o aviso do Vanguard some junto com ele.
 */
export function VanguardWaitNotice({ until }: { until: number }) {
  return (
    <section role="status" aria-label="Espera do Vanguard"
      className="flex max-w-[26rem] flex-col gap-1 rounded-card border border-warn bg-surface card p-4 text-left">
      <h3 className="text-body font-semibold text-ink">Espere até as {clockTime(until)} para abrir o LoL</h3>
      <p className="text-body text-muted">
        O Vanguard recusou a última abertura porque o LoL foi aberto muitas vezes em pouco tempo (erro VAN 216).
        Abrir antes desse horário faz a espera de 30 minutos começar de novo.
      </p>
    </section>
  );
}

/**
 * Fecha e abre o LoL (#579): foi o que trouxe o Vanguard de volta no caso do
 * #576. Com sucesso, o LoL fecha e o aviso some junto com a conexão.
 */
function RestartLeagueButton({ send }: { send: AgentClient['send'] }) {
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  async function restart() {
    if (inFlight.current) return;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: 'Fechando o LoL para abrir de novo…' });
    try {
      const result = await send({ type: 'restartLeague' });
      if (!mounted.current) return;
      if (result.status === 'ok') setFeedback({ status: 'idle', message: 'O LoL está abrindo de novo.' });
      else setFeedback({ status: 'error', message: result.code === 'invalidCommand'
        ? 'Não dá para reabrir o LoL com partida encontrada, na seleção de campeões nem na partida.'
        : result.code === 'notConnected' ? 'O LoL não está aberto.'
          : 'Não foi possível reabrir o LoL. Feche e abra pelo Riot Client.' });
    } catch {
      if (mounted.current) setFeedback({ status: 'error', message: 'Não foi possível confirmar. Confira se o LoL está abrindo antes de tentar de novo.' });
    } finally {
      inFlight.current = false;
    }
  }

  const busy = feedback.status === 'pending';
  return (
    <div className="flex flex-wrap items-center gap-3">
      <Button size="sm" variant="secondary" onClick={() => { void restart(); }} loading={busy}>
        {busy ? 'Reabrindo o LoL…' : 'Reabrir o LoL'}
      </Button>
      {feedback.message && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'}
          className={cx('text-caption', feedback.status === 'error' ? 'text-danger' : 'text-muted')}>
          {feedback.message}
        </p>
      )}
    </div>
  );
}
