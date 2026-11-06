import { useEffect, useRef, useState } from 'react';
import { mainScreen, phaseLabel } from './screens/mainScreen';
import { LobbyPanel } from './components/LobbyPanel';
import { useAgent } from './agent/useAgent';
import type { ConnectionSnapshot } from './agent/client';
import { QueuePanel } from './components/QueuePanel';
import { RunePagesPanel } from './components/RunePagesPanel';
import { ReadyCheckPanel } from './components/ReadyCheckPanel';
import { FriendsList } from './components/FriendsList';
import { InvitationsPanel } from './components/InvitationsPanel';
import { ChatButton, ChatPanel } from './components/ChatPanel';
import { Notifications } from './components/Notifications';
import { ChampSelectPanel } from './components/ChampSelectPanel';
import { TitleBar } from './components/titlebar/TitleBar';
import { useTheme } from './theme/theme';
import { useSidebar } from './layout/sidebar';
import { SidebarToggle } from './layout/SidebarToggle';

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
  const { preference, setPreference } = useTheme();
  const [chatWith, setChatWith] = useState<string | null>(null);
  const screen = mainScreen(connection);
  const sidebar = useSidebar(screen === 'champSelect');
  const invitations = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.receivedInvitations.length : 0;
  const contentRef = useRef<HTMLElement>(null);
  const flowButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
    if (document.activeElement?.closest('[hidden]')) flowButtonRef.current?.focus();
  }, [screen]);
  const [navigation, setNavigation] = useState({ screen, page: 'main' });
  // Uma nova fase confirmada traz o fluxo principal de volta ao primeiro plano.
  if (navigation.screen !== screen) setNavigation({ screen, page: 'main' });
  const page = navigation.screen === screen ? navigation.page : 'main';
  const found = screen === 'found';
  const playable = screen === 'home' || screen === 'lobby' || screen === 'queue';
  const title = screen === 'lobby' ? 'Lobby' : screen === 'queue' ? 'Fila'
    : found ? 'Partida encontrada' : screen === 'champSelect' ? 'Seleção de campeões'
      : screen === 'other' ? 'Partida' : 'Home';
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  const name = summoner && (summoner.gameName
    ? summoner.gameName + (summoner.tagLine ? '#' + summoner.tagLine : '')
    : summoner.displayName || 'Invocador');
  return (
    <div className="app-shell">
      <TitleBar connection={connection} send={connection.send} themePreference={preference} onThemeChange={setPreference} />
      <div className="app-body">
        <main ref={contentRef} className="app-content">
          <p role="status" className="text-slate-300">{connectionLabel(connection)}</p>
          {connection.error && <p role="alert" className="text-amber-300">{connection.error}</p>}
          <div className="flex flex-wrap items-start justify-between gap-2">
            <nav aria-label="Funcionalidades" className="flex flex-wrap gap-2">
              <button ref={flowButtonRef} type="button" aria-current={page === 'main' ? 'page' : undefined}
                onClick={() => setNavigation({ screen, page: 'main' })}>{title}</button>
              {screen !== 'home' && <button type="button" aria-current={page === 'home' ? 'page' : undefined}
                onClick={() => setNavigation({ screen, page: 'home' })}>Home</button>}
              <button type="button" aria-current={page === 'runes' ? 'page' : undefined}
                onClick={() => setNavigation({ screen, page: 'runes' })}>Runas</button>
            </nav>
            <SidebarToggle open={sidebar.open} onToggle={sidebar.toggle} invitations={invitations} />
          </div>
          {found && !connection.state?.readyCheck && <section role="alert" className="rounded border-2 border-amber-400 p-4">
            <h2 className="text-xl font-semibold">Partida encontrada</h2>
            <p>Aguardando os dados de confirmação do agente. Use o botão acima para abrir o client oficial.</p>
          </section>}
          {found && <ReadyCheckPanel connection={connection} send={connection.send} />}
          <section hidden={page !== 'home' && !(page === 'main' && screen === 'home')} aria-label="Home" className="space-y-4">
            <h2 className="text-2xl font-semibold">Home</h2>
            {summoner && <section aria-label="Invocador logado">
              <h3 className="break-words text-xl font-semibold">{name}</h3>
              <p className="text-slate-300">Nível {summoner.summonerLevel}</p>
            </section>}
            <p>Organize seu lobby e suas runas. O andamento da partida é atualizado pelo agente.</p>
            {screen !== 'home' && <button type="button" onClick={() => setNavigation({ screen, page: 'main' })}>Voltar para {title}</button>}
          </section>
          {page === 'main' && screen !== 'home' && !found && <h2 className="text-2xl font-semibold">{title}</h2>}
          {page === 'main' && screen === 'other' && <p>{phaseLabel(connection.state?.gameflowPhase)} Use o client oficial para continuar.</p>}
          {/* Posições estáveis: navegar ou mudar de tela não recria comandos em andamento. */}
          <div hidden={page !== 'main' || !playable} className="play-panels" data-screen={screen}>
            <div hidden={screen === 'queue'}>
              <LobbyPanel connection={connection} send={connection.send} />
            </div>
            <div hidden={screen === 'home'}>
              <QueuePanel connection={connection} send={connection.send} />
            </div>
          </div>
          {/* Montada fora da tela também, para a busca e as ações em andamento sobreviverem à navegação. */}
          <div hidden={page !== 'main' || screen !== 'champSelect'}>
            <ChampSelectPanel connection={connection} send={connection.send} />
          </div>
          <div hidden={page !== 'runes'}>
            <RunePagesPanel connection={connection} send={connection.send} />
          </div>
        </main>
        {/* Sempre montada: escondida, a coluna mantém conversas e convites em andamento. */}
        <aside id="coluna-amigos" aria-label="Amigos e conversas" hidden={!sidebar.open} className="app-sidebar">
          <InvitationsPanel connection={connection} send={connection.send} />
          <FriendsList connection={connection} renderActions={friend => <ChatButton friend={friend} onOpen={setChatWith} />} />
          <ChatPanel connection={connection} send={connection.send} activeId={chatWith} onActiveChange={setChatWith} />
        </aside>
      </div>
      <Notifications connection={connection} />
    </div>
  );
}
