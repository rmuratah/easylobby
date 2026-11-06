import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import { readyCheckLabels, responseLabels } from '../agent/readyCheck';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
export function ReadyCheckPanel({ connection, send }: Props) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected' || !state.readyCheck) return null;
  const check = state.readyCheck;
  // Cada contexto tem seu próprio ciclo de vida; promessas antigas não atualizam o novo painel.
  return <ReadyCheckActions key={`${state.lcu.session}:${state.summoner?.puuid}:${state.gameflowPhase}:${check.state}:${check.playerResponse}`}
    check={check} active={state.gameflowPhase === 'ReadyCheck'} send={send} />;
}

function Countdown({ check }: { check: ReadyCheck }) {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    const started = Date.now();
    const interval = setInterval(() => setElapsed((Date.now() - started) / 1000), 250);
    return () => clearInterval(interval);
  }, []);
  return <strong aria-label="Tempo para responder">{Math.ceil(Math.max(0, check.duration - check.timer - elapsed))} s</strong>;
}

function ReadyCheckActions({ check, active, send }: { check: ReadyCheck; active: boolean; send: AgentClient['send'] }) {
  const [feedback, setFeedback] = useState('');
  const [error, setError] = useState(false);
  const [pending, setPending] = useState(false);
  const locked = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const canAnswer = active && check.state === 'InProgress' && check.playerResponse === 'None';
  async function answer(type: 'acceptReadyCheck' | 'declineReadyCheck') {
    if (!canAnswer || locked.current) return;
    locked.current = true;
    setPending(true);
    setError(false);
    setFeedback('Enviando resposta…');
    try {
      const result = await send({ type });
      if (!mounted.current) return;
      if (result.status === 'error') {
        locked.current = false;
        setPending(false);
        setError(true);
        setFeedback(`Não foi possível responder: ${result.message}`);
      } else {
        setFeedback('Resposta enviada. Aguardando confirmação do agente…');
      }
    } catch {
      if (!mounted.current) return;
      // Timeout/falha de transporte não provam que a LCU deixou de executar o comando.
      setError(true);
      setFeedback('Não foi possível confirmar o envio. Aguarde o agente ou confira no client oficial.');
    }
  }
  return <section aria-label="Partida encontrada" className="flex w-full max-w-lg flex-col gap-3 rounded border border-sky-400 p-4">
    <h2 className="text-xl font-semibold">Partida encontrada</h2>
    <p role={check.state === 'Error' ? 'alert' : 'status'}>Estado: {readyCheckLabels[check.state]}</p>
    <p>Resposta confirmada: <strong>{responseLabels[check.playerResponse]}</strong></p>
    {active && check.state === 'InProgress' && <p>Tempo para responder: <Countdown key={`${check.timer}:${check.duration}`} check={check} /></p>}
    <div className="flex gap-3">
      <button type="button" className="rounded border px-4 py-2 disabled:opacity-50" disabled={!canAnswer || pending}
        onClick={() => { void answer('acceptReadyCheck'); }}>Aceitar</button>
      <button type="button" className="rounded border px-4 py-2 disabled:opacity-50" disabled={!canAnswer || pending}
        onClick={() => { void answer('declineReadyCheck'); }}>Recusar</button>
    </div>
    {feedback && <p role={error ? 'alert' : 'status'}>{feedback}</p>}
  </section>;
}
