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

/** Quanto tempo o botão espera o LoL conectar antes de deixar pedir de novo (#665). */
export const LAUNCH_WAIT_MS = 90_000;

function LaunchLeague({ send }: { send: AgentClient['send'] }) {
  const [state, setState] = useState<{ status: 'idle' | 'pending' | 'opening' | 'error'; message: string }>({ status: 'idle', message: '' });
  const mounted = useRef(true);
  const inFlight = useRef(false);
  useEffect(() => () => { mounted.current = false; }, []);
  // Pedido aceito: o botão fica bloqueado até esta tela sair (o LoL conectou).
  // Se o LoL não conectar a tempo (login fechado no Riot Client), libera de novo.
  useEffect(() => {
    if (state.status !== 'opening') return;
    const timer = setTimeout(() => {
      inFlight.current = false;
      setState({ status: 'idle', message: 'O League of Legends ainda não conectou. Confira o Riot Client ou tente abrir de novo.' });
    }, LAUNCH_WAIT_MS);
    return () => { clearTimeout(timer); };
  }, [state.status]);

  async function launch() {
    if (inFlight.current) return;
    inFlight.current = true;
    setState({ status: 'pending', message: '' });
    try {
      const result = await send({ type: 'launchLeague' });
      if (!mounted.current) return;
      if (result.status === 'ok') {
        setState({ status: 'opening', message: 'Abrindo o League of Legends. Se o Riot Client pedir login, entre na sua conta e o LoL abre sozinho. Esta tela muda quando ele conectar.' });
        return;
      }
      inFlight.current = false;
      setState({ status: 'error', message: 'Não foi possível abrir o League of Legends. Abra pelo Riot Client e o easylobby conecta sozinho.' });
    } catch {
      inFlight.current = false;
      if (mounted.current) setState({ status: 'error', message: 'Não foi possível pedir a abertura. Abra pelo Riot Client e o easylobby conecta sozinho.' });
    }
  }

  const busy = state.status === 'pending' || state.status === 'opening';
  return (
    <div className="flex flex-col items-center gap-3">
      <Button variant="primary" size="lg" loading={busy} onClick={() => { void launch(); }}>
        {state.status === 'opening' ? 'Abrindo o League of Legends…' : 'Abrir o League of Legends'}
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
