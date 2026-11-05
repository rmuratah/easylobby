import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };

export function LaunchUxButton({ connection, send }: Props) {
  const lcu = connection.state?.lcu;
  const available = connection.status === 'connected' && lcu?.state === 'connected';
  // Uma nova sessão não deve herdar resultados nem requisições da anterior.
  const session = available ? lcu.session : 'unavailable';
  return <LaunchUxAction key={session} available={available} send={send} />;
}

function LaunchUxAction({ available, send }: { available: boolean; send: AgentClient['send'] }) {
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

  return (
    <div className="flex max-w-lg flex-col items-center gap-2 text-center">
      <button
        type="button"
        onClick={() => { void launch(); }}
        disabled={!available || feedback.status === 'pending'}
        aria-busy={feedback.status === 'pending'}
        aria-describedby={descriptionId}
        className="rounded border border-amber-400 px-4 py-2 font-medium text-amber-200 hover:bg-amber-400/10 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-amber-300 disabled:cursor-not-allowed disabled:opacity-50"
      >
        {feedback.status === 'pending' ? 'Abrindo client oficial…' : 'Abrir client oficial'}
      </button>
      <p id={descriptionId} className="text-sm text-slate-300">
        {available ? 'Use esta opção para voltar à interface oficial do League of Legends.'
          : 'Aguarde a conexão com o agente e o League of Legends. Se havia uma abertura em andamento, confira o client oficial antes de tentar novamente.'}
      </p>
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}>{feedback.message}</p>}
    </div>
  );
}
