import { useEffect, useRef, useState } from 'react';
import { mainScreen, phaseLabel } from './screens/mainScreen';
import { LobbyPanel } from './components/LobbyPanel';
import { useAgent } from './agent/useAgent';
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
import { connectionView } from './screens/connection';
import { ConnectionScreen } from './components/connection/ConnectionScreen';
import { SummonerHeader } from './components/home/SummonerHeader';
import { EmptyState } from './components/base';

export default function App() {
  const connection = useAgent();
  const { preference, setPreference } = useTheme();
  const [chatWith, setChatWith] = useState<string | null>(null);
  const screen = mainScreen(connection);
  const view = connectionView(connection);
  const ready = view.screen === null;
  const sidebar = useSidebar(screen === 'champSelect');
  const invitations = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.receivedInvitations.length : 0;
  const contentRef = useRef<HTMLElement>(null);
  const flowButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
    if (document.activeElement?.closest('[hidden]')) flowButtonRef.current?.focus();
  }, [screen, ready]);
  const [navigation, setNavigation] = useState({ screen, page: 'main' });
  // Uma nova fase confirmada traz o fluxo principal de volta ao primeiro plano.
  if (navigation.screen !== screen) setNavigation({ screen, page: 'main' });
  const page = navigation.screen === screen ? navigation.page : 'main';
  const found = screen === 'found';
  const playable = screen === 'home' || screen === 'lobby' || screen === 'queue';
  const title = screen === 'lobby' ? 'Lobby' : screen === 'queue' ? 'Fila'
    : found ? 'Partida encontrada' : screen === 'champSelect' ? 'Seleção de campeões'
      : screen === 'other' ? 'Partida' : 'Início';
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  return (
    <div className="app-shell">
      <TitleBar connection={connection} send={connection.send} themePreference={preference} onThemeChange={setPreference} />
      <div className="app-body">
        <main ref={contentRef} className="app-content">
          {view.screen && <ConnectionScreen view={view.screen} send={connection.send} error={connection.error} />}
          {/* Escondido, e não desmontado, enquanto reconecta: comandos em andamento sobrevivem à queda. */}
          <div hidden={!ready} className="flex flex-col gap-5">
            {ready && connection.error && <p role="alert" className="text-warn">{connection.error}</p>}
            <div className="flex flex-wrap items-start justify-between gap-2">
              <nav aria-label="Funcionalidades" className="flex flex-wrap gap-2">
                <button ref={flowButtonRef} type="button" aria-current={page === 'main' ? 'page' : undefined}
                  onClick={() => setNavigation({ screen, page: 'main' })}>{title}</button>
                {screen !== 'home' && <button type="button" aria-current={page === 'home' ? 'page' : undefined}
                  onClick={() => setNavigation({ screen, page: 'home' })}>Início</button>}
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
            <section hidden={page !== 'home' && !(page === 'main' && screen === 'home')} aria-label="Início" className="flex flex-col gap-4">
              {summoner && <SummonerHeader summoner={summoner} />}
              {screen !== 'home' && <button type="button" className="nav-button self-start" onClick={() => setNavigation({ screen, page: 'main' })}>Voltar para {title}</button>}
            </section>
            {page === 'main' && screen !== 'home' && screen !== 'other' && !found && <h2 className="text-2xl font-semibold">{title}</h2>}
            {page === 'main' && screen === 'other' && <EmptyState title={phaseLabel(connection.state?.gameflowPhase)}
              description="Esta fase ainda não tem tela no easylobby. Use “Abrir client oficial”, no alto da janela, para continuar." />}
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
