import { useEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import { readyCheckLabels, responseLabels } from '../agent/readyCheck';
import { Button } from './base';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
export function ReadyCheckPanel({ connection, send }: Props) {
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected' || !state.readyCheck) return null;
  const check = state.readyCheck;
  // Cada contexto tem seu próprio ciclo de vida; promessas antigas não atualizam o novo painel.
  return <ReadyCheckActions key={`${state.lcu.session}:${state.summoner?.puuid}:${state.gameflowPhase}:${check.state}:${check.playerResponse}`}
    check={check} active={state.gameflowPhase === 'ReadyCheck'} send={send} />;
}

// Contador e barra usam o mesmo relógio: o timer da LCU mais o tempo desde o último evento.
function useElapsed(check: ReadyCheck) {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    const started = Date.now();
    const interval = setInterval(() => setElapsed((Date.now() - started) / 1000), 250);
    return () => clearInterval(interval);
  }, []);
  return Math.min(check.duration, Math.max(0, check.timer + elapsed));
}

function Countdown({ check }: { check: ReadyCheck }) {
  const elapsed = useElapsed(check);
  return <strong aria-label="Tempo para responder">{Math.ceil(check.duration - elapsed)} s</strong>;
}

// Preenche da esquerda para a direita até acabar o tempo (#347).
function TimeBar({ check }: { check: ReadyCheck }) {
  const elapsed = useElapsed(check);
  const percent = check.duration > 0 ? (elapsed / check.duration) * 100 : 100;
  return <div role="progressbar" aria-label="Tempo decorrido para responder" aria-valuemin={0}
    aria-valuemax={check.duration} aria-valuenow={Math.floor(elapsed)}
    className="h-2 w-full overflow-hidden rounded-full bg-control">
    <div data-testid="ready-check-bar" className="h-full rounded-full bg-accent transition-[width] duration-[250ms] ease-linear"
      style={{ width: `${percent}%` }} />
  </div>;
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
  // Enter aceita (#129), a não ser que o foco esteja num campo ou botão, que já tratam o Enter.
  const acceptRef = useRef(answer);
  useEffect(() => { acceptRef.current = answer; });
  useEffect(() => {
    if (!canAnswer) return;
    function onKey(event: KeyboardEvent) {
      if (event.key !== 'Enter' || event.repeat || event.defaultPrevented) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, textarea, select, button, a, [contenteditable="true"]')) return;
      event.preventDefault();
      void acceptRef.current('acceptReadyCheck');
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [canAnswer]);
  const answering = active && check.state === 'InProgress';
  return <section aria-label="Partida encontrada"
    className="sticky bottom-0 z-10 flex w-full max-w-[56rem] flex-col gap-4 rounded-card border-2 border-accent bg-accent-soft p-5 text-ink shadow-[0_-8px_24px_-12px_var(--el-overlay)]">
    <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <h2 className="text-title font-semibold tracking-[-0.015em] text-accent-ink">Partida encontrada</h2>
        <p role={check.state === 'Error' ? 'alert' : 'status'} className="text-body">Estado: {readyCheckLabels[check.state]}</p>
        <p className="text-body text-muted">Resposta confirmada: <strong className="font-semibold text-ink">{responseLabels[check.playerResponse]}</strong></p>
      </div>
      {answering && <p className="flex flex-col items-center text-caption text-muted">Tempo para responder
        <span className="text-[2.5rem] font-semibold leading-none tabular-nums text-ink"><Countdown key={`${check.timer}:${check.duration}`} check={check} /></span></p>}
    </div>
    {answering && <TimeBar key={`${check.timer}:${check.duration}`} check={check} />}
    <div className="flex flex-wrap items-center gap-3">
      <Button variant="primary" size="lg" className="min-w-[10rem]" disabled={!canAnswer || pending}
        aria-keyshortcuts="Enter" onClick={() => { void answer('acceptReadyCheck'); }}>Aceitar</Button>
      <Button variant="secondary" size="lg" disabled={!canAnswer || pending}
        onClick={() => { void answer('declineReadyCheck'); }}>Recusar</Button>
      {canAnswer && <span className="text-caption text-muted">Enter aceita</span>}
    </div>
    {feedback && <p role={error ? 'alert' : 'status'} className={error ? 'text-body text-danger' : 'text-body text-muted'}>{feedback}</p>}
  </section>;
}
