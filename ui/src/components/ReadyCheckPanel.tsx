import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import type { ReadyCheck } from '../bindings/ReadyCheck';
import { readyCheckLabels, responseLabels } from '../agent/readyCheck';
import { Button } from './base';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
export function ReadyCheckPanel({ connection, send }: Props) {
  // O relógio fica aqui para sobreviver às remontagens de ReadyCheckActions (ex.: ao aceitar).
  const [clock] = useState(createClock);
  const state = connection.state;
  if (connection.status !== 'connected' || state?.lcu.state !== 'connected' || !state.readyCheck) return null;
  const check = state.readyCheck;
  // Cada contexto tem seu próprio ciclo de vida; promessas antigas não atualizam o novo painel.
  return <ReadyCheckActions key={`${state.lcu.session}:${state.summoner?.puuid}:${state.gameflowPhase}:${check.state}:${check.playerResponse}`}
    check={check} active={state.gameflowPhase === 'ReadyCheck'} send={send} clock={clock} />;
}

// Contador e barra usam o mesmo relógio, ancorado no timer da LCU. O timer chega de segundo
// em segundo, então o tempo real fica entre `timer` e `timer + 1`. Só reancoramos quando o
// relógio local sai dessa faixa (outro ready check, atraso ou evento adiantado); assim a barra
// corre lisa entre os eventos em vez de recomeçar a cada um (#422).
type Clock = { sync(check: ReadyCheck): void; elapsed(): number };
function createClock(): Clock {
  let start: number | null = null;
  let duration = 0;
  const elapsed = () => start === null ? 0 : Math.min(duration, Math.max(0, (Date.now() - start) / 1000));
  return {
    sync(check) {
      const now = Date.now();
      duration = check.duration;
      const local = start === null ? null : (now - start) / 1000;
      if (local === null || local < check.timer || local > check.timer + 1) start = now - check.timer * 1000;
    },
    elapsed,
  };
}

// Redesenha a cada quadro, direto no DOM, sem renderizar o React 60 vezes por segundo.
function useFrames(check: ReadyCheck, clock: Clock, draw: (elapsed: number) => void) {
  const drawRef = useRef(draw);
  useLayoutEffect(() => { drawRef.current = draw; });
  useLayoutEffect(() => {
    clock.sync(check);
    let frame = 0;
    const tick = () => { drawRef.current(clock.elapsed()); frame = requestAnimationFrame(tick); };
    tick();
    return () => cancelAnimationFrame(frame);
  }, [check, clock]);
}

function Countdown({ check, clock }: { check: ReadyCheck; clock: Clock }) {
  const ref = useRef<HTMLElement>(null);
  useFrames(check, clock, elapsed => {
    const text = `${Math.ceil(check.duration - elapsed)} s`;
    if (ref.current && ref.current.textContent !== text) ref.current.textContent = text;
  });
  return <strong ref={ref} aria-label="Tempo para responder" />;
}

// Preenche da esquerda para a direita até acabar o tempo (#347).
function TimeBar({ check, clock }: { check: ReadyCheck; clock: Clock }) {
  const bar = useRef<HTMLDivElement>(null);
  const fill = useRef<HTMLDivElement>(null);
  useFrames(check, clock, elapsed => {
    const percent = check.duration > 0 ? (elapsed / check.duration) * 100 : 100;
    if (fill.current) fill.current.style.width = `${percent}%`;
    bar.current?.setAttribute('aria-valuenow', String(Math.floor(elapsed)));
  });
  return <div ref={bar} role="progressbar" aria-label="Tempo decorrido para responder" aria-valuemin={0}
    aria-valuemax={check.duration} className="h-2 w-full overflow-hidden rounded-full bg-control">
    <div ref={fill} data-testid="ready-check-bar" className="h-full rounded-full bg-accent" />
  </div>;
}

function ReadyCheckActions({ check, active, send, clock }: { check: ReadyCheck; active: boolean; send: AgentClient['send']; clock: Clock }) {
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
        <span className="text-[2.5rem] font-semibold leading-none tabular-nums text-ink"><Countdown check={check} clock={clock} /></span></p>}
    </div>
    {answering && <TimeBar check={check} clock={clock} />}
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
