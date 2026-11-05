import { useAgent } from './agent/useAgent';
import type { ConnectionSnapshot } from './agent/client';
import { LaunchUxButton } from './components/LaunchUxButton';

function connectionLabel(connection: ConnectionSnapshot): string {
  if (connection.status === 'incompatible') return 'Versões incompatíveis';
  if (connection.status === 'reconnecting') return 'Reconectando ao agente…';
  if (!connection.state) return 'Conectando ao agente…';
  const lcu = connection.state.lcu;
  switch (lcu.state) {
    case 'starting': return 'Aguardando o League of Legends…';
    case 'connected': return 'Conectado ao League of Legends';
    case 'disconnected': return 'Conexão com o League of Legends interrompida';
    case 'retrying': return lcu.clientNotRunning ? 'Abra o League of Legends para continuar' : 'Tentando reconectar ao League of Legends…';
  }
}

export default function App() {
  const connection = useAgent();
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 bg-slate-950 p-6 text-slate-100">
      <h1 className="text-3xl font-semibold">easylobby</h1>
      <LaunchUxButton connection={connection} send={connection.send} />
      <p role="status" className="text-center text-slate-300">{connectionLabel(connection)}</p>
      {connection.error && <p role="alert" className="max-w-lg text-center text-amber-300">{connection.error}</p>}
    </main>
  );
}
