import { useEffect, useRef, useState } from 'react';
import type { AgentClient } from '../../agent/client';
import type { ConnectionView } from '../../screens/connection';
import { Button, EmptyState, Spinner } from '../base';

type Props = { view: NonNullable<ConnectionView['screen']>; send: AgentClient['send']; error: string | null };

/** Ocupa a área principal enquanto o app não está pronto para jogar (#128). */
export function ConnectionScreen({ view, send, error }: Props) {
  const icon = view.kind === 'connecting' ? undefined : view.kind === 'lolClosed' ? <PlayIcon /> : <AlertIcon />;
  return (
    <div className="flex min-h-full flex-1 flex-col items-center justify-center">
      {view.kind === 'connecting' && <Spinner size={40} label={null} />}
      <EmptyState
        className="pt-4"
        icon={icon}
        title={view.title}
        description={<>{view.description}{error && <span role="alert" className="mt-2 block text-warn">{error}</span>}</>}
        action={view.kind === 'lolClosed' ? <LaunchLeague send={send} /> : undefined}
      />
    </div>
  );
}

function LaunchLeague({ send }: { send: AgentClient['send'] }) {
  const [state, setState] = useState<{ status: 'idle' | 'pending' | 'opening' | 'error'; message: string }>({ status: 'idle', message: '' });
  const mounted = useRef(true);
  useEffect(() => () => { mounted.current = false; }, []);

  async function launch() {
    setState({ status: 'pending', message: '' });
    try {
      const result = await send({ type: 'launchLeague' });
      if (!mounted.current) return;
      setState(result.status === 'ok'
        ? { status: 'opening', message: 'Abrindo o League of Legends. Esta tela muda sozinha quando ele conectar.' }
        : { status: 'error', message: 'Não foi possível abrir o League of Legends. Abra pelo Riot Client e o easylobby conecta sozinho.' });
    } catch {
      if (mounted.current) setState({ status: 'error', message: 'Não foi possível pedir a abertura. Abra pelo Riot Client e o easylobby conecta sozinho.' });
    }
  }

  return (
    <div className="flex flex-col items-center gap-3">
      <Button variant="primary" size="lg" loading={state.status === 'pending'} onClick={() => { void launch(); }}>
        Abrir o League of Legends
      </Button>
      {state.message && (
        <p role={state.status === 'error' ? 'alert' : undefined} aria-live="polite"
          className={state.status === 'error' ? 'max-w-[22rem] text-body text-danger' : 'max-w-[22rem] text-body text-muted'}>
          {state.message}
        </p>
      )}
    </div>
  );
}

function PlayIcon() {
  return <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M5 12h14M12 5l7 7-7 7" /></svg>;
}

function AlertIcon() {
  return <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round"><path d="M12 8v5M12 16.5v.01M10.3 3.9 2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z" /></svg>;
}
