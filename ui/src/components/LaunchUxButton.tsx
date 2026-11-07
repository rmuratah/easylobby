import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { Button } from './base';
import { cx } from './base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send']; compact?: boolean };
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };

export function LaunchUxButton({ connection, send, compact = false }: Props) {
  const lcu = connection.state?.lcu;
  const available = connection.status === 'connected' && lcu?.state === 'connected';
  // Uma nova sessão não deve herdar resultados nem requisições da anterior.
  const session = available ? lcu.session : 'unavailable';
  return <LaunchUxAction key={session} available={available} send={send} compact={compact} />;
}

function LaunchUxAction({ available, send, compact }: { available: boolean; send: AgentClient['send']; compact: boolean }) {
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const descriptionId = useId();
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  async function launch() {
    if (!available || inFlight.current) return;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: 'Abrindo o client oficial…' });
    try {
      const result = await send({ type: 'launchUx' });
      if (!mounted.current) return;
      if (result.status === 'ok') {
        setFeedback({ status: 'success', message: 'Abertura do client oficial confirmada.' });
      } else {
        setFeedback({ status: 'error', message: result.code === 'notConnected'
          ? 'O League of Legends não está conectado. Aguarde a reconexão e tente novamente.'
          : 'Não foi possível abrir o client oficial. Tente novamente.' });
      }
    } catch {
      if (!mounted.current) return;
      setFeedback({ status: 'error', message: 'Não foi possível confirmar a abertura. Confira o client oficial e a conexão antes de tentar novamente.' });
    } finally {
      inFlight.current = false;
    }
  }

  const busy = feedback.status === 'pending';
  // Fica nos controles da janela (#126): a explicação vai para leitores de tela
  // e para a dica do mouse; o resultado aparece em uma linha ao lado do botão.
  const description = available ? 'Use esta opção para voltar à interface oficial do League of Legends.'
    : 'Aguarde a conexão com o agente e o League of Legends. Se havia uma abertura em andamento, confira o client oficial antes de tentar novamente.';
  return (
    <div className="flex min-w-0 items-center gap-2">
      {feedback.message && (
        <p role={feedback.status === 'error' ? 'alert' : 'status'} title={feedback.message}
          className={cx('min-w-0 max-w-[16rem] truncate text-caption', feedback.status === 'error' ? 'text-danger' : 'text-muted')}>
          {feedback.message}
        </p>
      )}
      <Button size="sm" variant="secondary" onClick={() => { void launch(); }} disabled={!available}
        loading={busy} aria-label={busy ? 'Abrindo client oficial…' : 'Abrir client oficial'}
        aria-describedby={descriptionId} title={description} className={compact ? 'w-9 px-0' : undefined}>
        {!busy && compact && <svg aria-hidden="true" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M14 5h5v14h-5M10 17l5-5-5-5M15 12H3" /></svg>}
        <span className={compact ? 'sr-only' : undefined}>{busy ? 'Abrindo client oficial…' : 'Abrir client oficial'}</span>
      </Button>
      <span id={descriptionId} className="sr-only">{description}</span>
    </div>
  );
}
