import { LobbyPanel } from './components/LobbyPanel';
import { useAgent } from './agent/useAgent';
import type { ConnectionSnapshot } from './agent/client';
import { LaunchUxButton } from './components/LaunchUxButton';
import { QueuePanel } from './components/QueuePanel';
import { RunePagesPanel } from './components/RunePagesPanel';

function connectionLabel(connection: ConnectionSnapshot): string {
  if (connection.status === 'incompatible') return 'Versões incompatíveis';
  if (connection.status === 'reconnecting') return 'Reconectando ao agente…';
  if (!connection.state) return 'Conectando ao agente…';
  const lcu = connection.state.lcu;
  switch (lcu.state) {
    case 'starting': return 'Aguardando o League of Legends…';
    case 'connected': return connection.state.summoner ? 'Conectado ao League of Legends' : 'Aguardando login no League of Legends…';
    case 'disconnected': return 'Conexão com o League of Legends interrompida';
    case 'retrying': return lcu.clientNotRunning ? 'Abra o League of Legends para continuar' : 'Tentando reconectar ao League of Legends…';
  }
}

export default function App() {
  const connection = useAgent();
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  const name = summoner && (summoner.gameName
    ? summoner.gameName + (summoner.tagLine ? '#' + summoner.tagLine : '')
    : summoner.displayName || 'Invocador');
  return (
    <main className="flex min-h-screen flex-col items-center justify-center gap-4 bg-slate-950 p-6 text-slate-100">
      <h1 className="text-3xl font-semibold">easylobby</h1>
      <LaunchUxButton connection={connection} send={connection.send} />
      <p role="status" className="text-center text-slate-300">{connectionLabel(connection)}</p>
      {summoner && (
        <section aria-label="Invocador logado" className="max-w-full text-center">
          <h2 className="break-words text-xl font-semibold">{name}</h2>
          <p className="text-slate-300">Nível {summoner.summonerLevel}</p>
        </section>
      )}
      {connection.error && <p role="alert" className="max-w-lg text-center text-amber-300">{connection.error}</p>}
      <LobbyPanel connection={connection} send={connection.send} />
      <QueuePanel connection={connection} send={connection.send} />
      <RunePagesPanel connection={connection} send={connection.send} />
    </main>
  );
}
