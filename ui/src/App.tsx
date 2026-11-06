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
import { totalUnread } from './agent/chat';

export default function App() {
  const connection = useAgent();
  const { theme, setPreference } = useTheme();
  const [chatWith, setChatWith] = useState<string | null>(null);
  const screen = mainScreen(connection);
  const view = connectionView(connection);
  const ready = view.screen === null;
  const sidebar = useSidebar(screen === 'champSelect');
  const invitations = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.receivedInvitations.length : 0;
  const unread = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? totalUnread(connection.state.conversations) : 0;
  const contentRef = useRef<HTMLElement>(null);
  const flowButtonRef = useRef<HTMLButtonElement>(null);
  const runesBackRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
    if (document.activeElement?.closest('[hidden]')) flowButtonRef.current?.focus();
  }, [screen, ready]);
  const [navigation, setNavigation] = useState({ screen, page: 'main' });
  // Uma nova fase confirmada traz o fluxo principal de volta ao primeiro plano.
  if (navigation.screen !== screen) setNavigation({ screen, page: 'main' });
  const page = navigation.screen === screen ? navigation.page : 'main';
  // Quem trocou de página por um botão que agora ficou escondido não perde o foco.
  useEffect(() => {
    if (!document.activeElement?.closest('[hidden]')) return;
    (page === 'runes' ? runesBackRef.current ?? flowButtonRef.current : flowButtonRef.current)?.focus();
  }, [page]);
  const found = screen === 'found';
  // Início (#128) e lobby (#129) no layout de referência: cartões lado a lado quando há largura.
  const homeMain = page === 'main' && screen === 'home';
  const lobbyMain = screen === 'lobby' || screen === 'queue';
  const playable = screen === 'home' || screen === 'lobby' || screen === 'queue' || found;
  const title = screen === 'lobby' ? 'Lobby' : screen === 'queue' ? 'Fila'
    : found ? 'Partida encontrada' : screen === 'champSelect' ? 'Seleção de campeões'
      : screen === 'other' ? 'Partida' : 'Início';
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  return (
    <div className="app-shell">
      <TitleBar connection={connection} send={connection.send} theme={theme} onThemeChange={setPreference} />
      <div className="app-body">
        <main ref={contentRef} className="app-content">
          {view.screen && <ConnectionScreen view={view.screen} send={connection.send} error={connection.error} />}
          {/* Escondido, e não desmontado, enquanto reconecta: comandos em andamento sobrevivem à queda. */}
          <div hidden={!ready} className={homeMain
            ? 'grid max-w-[72rem] grid-flow-row-dense grid-cols-1 gap-5 @min-[44rem]:grid-cols-[minmax(0,2fr)_minmax(15rem,1fr)]'
            : 'flex flex-col gap-5'}>
            {ready && connection.error && <p role="alert" className="col-span-full text-warn">{connection.error}</p>}
            <div className="col-span-full flex flex-wrap items-start justify-between gap-2">
              <nav aria-label="Funcionalidades" className="flex flex-wrap gap-2">
                <button ref={flowButtonRef} type="button" aria-current={page === 'main' ? 'page' : undefined}
                  onClick={() => setNavigation({ screen, page: 'main' })}>{title}</button>
                {screen !== 'home' && <button type="button" aria-current={page === 'home' ? 'page' : undefined}
                  onClick={() => setNavigation({ screen, page: 'home' })}>Início</button>}
                <button type="button" aria-current={page === 'runes' ? 'page' : undefined}
                  onClick={() => setNavigation({ screen, page: 'runes' })}>Runas</button>
              </nav>
              <SidebarToggle open={sidebar.open} onToggle={sidebar.toggle} invitations={invitations} unread={unread} />
            </div>
            <section hidden={page !== 'home' && !(page === 'main' && screen === 'home')} aria-label="Início"
              className={homeMain ? 'order-2 grid min-w-0 gap-4 @min-[44rem]:order-none @min-[44rem]:col-start-2' : 'flex max-w-[22rem] flex-col gap-4'}>
              {summoner && <SummonerHeader summoner={summoner} />}
              {homeMain && <div className="grid grid-cols-1 gap-4 @min-[22rem]:grid-cols-2">
                <article aria-label="Área reservada" className="flex min-h-32 flex-col justify-end rounded-panel surface p-5 shadow-soft animate-fade-up" style={{ animationDelay: '140ms' }}>
                  <h2 className="text-section font-semibold text-ink">Área reservada</h2>
                  <p className="mt-1 text-body text-muted">Espaço para uma futura funcionalidade.</p>
                </article>
                <article aria-label="Sem conteúdo" className="flex min-h-32 flex-col justify-end rounded-panel surface p-5 shadow-soft animate-fade-up" style={{ animationDelay: '200ms' }}>
                  <h2 className="text-section font-semibold text-ink">Sem conteúdo</h2>
                  <p className="mt-1 text-body text-muted">Espaço reservado no momento.</p>
                </article>
              </div>}
              {screen !== 'home' && <button type="button" className="nav-button self-start" onClick={() => setNavigation({ screen, page: 'main' })}>Voltar para {title}</button>}
            </section>
            {page === 'main' && screen === 'champSelect' && <h2 className="text-title font-semibold tracking-[-0.015em]">{title}</h2>}
            {page === 'main' && screen === 'other' && <EmptyState title={phaseLabel(connection.state?.gameflowPhase)}
              description="Esta fase ainda não tem tela no easylobby. Use “Abrir client oficial”, no alto da janela, para continuar." />}
            {/* Posições estáveis: navegar ou mudar de tela não recria comandos em andamento. */}
            {/* Tela unificada (#129): o lobby fica sempre à vista; a busca e a partida encontrada aparecem embaixo dele. */}
            <div hidden={page !== 'main' || !playable} className={homeMain ? 'order-1 @min-[44rem]:order-none @min-[44rem]:col-start-1'
              : lobbyMain ? 'grid max-w-[72rem] items-start gap-5 @min-[44rem]:grid-cols-[minmax(0,1fr)_18rem]'
              : 'flex max-w-[56rem] flex-col gap-4'}>
              <LobbyPanel connection={connection} send={connection.send} />
              {/* Coluna da direita com largura; embaixo, presa à borda, quando a janela é estreita. */}
              <div hidden={screen === 'home' || found} className="sticky bottom-0 z-10 @min-[44rem]:top-0 @min-[44rem]:bottom-auto">
                <QueuePanel connection={connection} send={connection.send} />
              </div>
            </div>
            {/* A partida encontrada fica no lugar da faixa de busca, logo abaixo do lobby, e continua visível nas outras páginas. */}
            {found && !connection.state?.readyCheck && <section role="alert"
              className="sticky bottom-0 z-10 flex max-w-[56rem] flex-col gap-1 rounded-card border-2 border-accent bg-accent-soft p-5 text-ink">
              <h2 className="text-title font-semibold tracking-[-0.015em] text-accent-ink">Partida encontrada</h2>
              <p className="text-body">Aguardando os dados de confirmação do agente. Use “Abrir client oficial”, no alto da janela, se precisar.</p>
            </section>}
            {found && <ReadyCheckPanel connection={connection} send={connection.send} />}
            {/* Montada fora da tela também, para a busca e as ações em andamento sobreviverem à navegação. */}
            <div hidden={page !== 'main' || screen !== 'champSelect'}>
              <ChampSelectPanel connection={connection} send={connection.send}
                onOpenRunes={() => setNavigation({ screen, page: 'runes' })} />
            </div>
            <div hidden={page !== 'runes'} className="flex flex-col gap-4">
              {/* Na seleção, a volta fica à mão: a seleção continua montada e o timer segue correndo (#131). */}
              {screen !== 'home' && <button ref={runesBackRef} type="button" className="nav-button self-start"
                onClick={() => setNavigation({ screen, page: 'main' })}>Voltar para {title}</button>}
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
      <Notifications connection={connection} quiet={screen === 'champSelect'} sidebarOpen={sidebar.open} />
    </div>
  );
}
