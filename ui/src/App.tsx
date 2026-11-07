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
import { SideNavigation, type NavigationPage } from './layout/SideNavigation';
import { SidebarToggle } from './layout/SidebarToggle';
import { connectionView } from './screens/connection';
import { ConnectionScreen } from './components/connection/ConnectionScreen';
import { SummonerHeader } from './components/home/SummonerHeader';
import { EmptyState } from './components/base';
import { totalUnread } from './agent/chat';

/** Runas não é destino da navegação lateral: o editor só abre pela seleção de campeões (#165). */
type Page = NavigationPage | 'runes';

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
  const [navigation, setNavigation] = useState<{ screen: typeof screen; page: 'main' | Page }>({ screen, page: 'main' });
  // Uma nova fase confirmada traz o fluxo principal de volta ao primeiro plano.
  if (navigation.screen !== screen) setNavigation({ screen, page: 'main' });
  const page = navigation.screen === screen ? navigation.page : 'main';
  const activePage: Page = page === 'main' ? (screen === 'home' ? 'home' : 'flow') : page;
  // Quem trocou de página por um botão que agora ficou escondido não perde o foco.
  useEffect(() => {
    if (!document.activeElement?.closest('[hidden]')) return;
    (activePage === 'runes' ? runesBackRef.current ?? flowButtonRef.current : flowButtonRef.current)?.focus();
  }, [activePage]);
  const found = screen === 'found';
  // Início (#128) e lobby (#129) no layout de referência: cartões lado a lado quando há largura.
  const playable = screen === 'home' || screen === 'lobby' || screen === 'queue' || found;
  const homeLayout = activePage === 'home' && playable;
  const lobbyMain = screen === 'lobby' || screen === 'queue';
  const title = screen === 'lobby' ? 'Lobby' : screen === 'queue' ? 'Fila'
    : found ? 'Partida encontrada' : screen === 'champSelect' ? 'Seleção de campeões'
      : screen === 'other' ? 'Partida' : 'Início';
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  return (
    <div className="app-shell">
      <TitleBar connection={connection} send={connection.send} />
      <div className={sidebar.open || !ready ? 'app-body' : 'app-body sidebar-collapsed'}>
        <div hidden={!ready} className="contents">
          <SideNavigation active={activePage === 'runes' ? 'flow' : activePage} flowLabel="Lobby" flowButtonRef={flowButtonRef} theme={theme}
            onNavigate={next => setNavigation({ screen, page: next })} onThemeChange={setPreference} />
        </div>
        <div className="app-main-column">
          <main ref={contentRef} className="app-content">
          {view.screen && <ConnectionScreen view={view.screen} send={connection.send} error={connection.error} />}
          {/* Escondido, e não desmontado, enquanto reconecta: comandos em andamento sobrevivem à queda. */}
          <div hidden={!ready} className={homeLayout ? 'home-grid' : 'flex min-h-full flex-col gap-5'}>
            {ready && connection.error && <p role="alert" className="col-span-full text-warn">{connection.error}</p>}
            <section hidden={activePage !== 'home'} aria-label="Início"
              className={homeLayout ? 'home-secondary' : 'flex max-w-[22rem] flex-col gap-4'}>
              {summoner && <SummonerHeader summoner={summoner} />}
              {homeLayout && <div className="home-placeholders">
                <article aria-label="Área reservada" className="flex min-h-32 flex-col justify-end rounded-panel surface p-5 shadow-soft animate-fade-up" style={{ animationDelay: '140ms' }}>
                  <h2 className="text-section font-semibold text-ink">Área reservada</h2>
                  <p className="mt-1 text-body text-muted">Espaço para uma futura funcionalidade.</p>
                </article>
                <article aria-label="Sem conteúdo" className="flex min-h-32 flex-col justify-end rounded-panel surface p-5 shadow-soft animate-fade-up" style={{ animationDelay: '200ms' }}>
                  <h2 className="text-section font-semibold text-ink">Sem conteúdo</h2>
                  <p className="mt-1 text-body text-muted">Espaço reservado no momento.</p>
                </article>
              </div>}
              {screen !== 'home' && !playable && <button type="button" className="nav-button self-start" onClick={() => setNavigation({ screen, page: 'flow' })}>Voltar para {title}</button>}
            </section>
            {activePage === 'flow' && screen === 'champSelect' && <h2 className="text-title font-semibold tracking-[-0.015em]">{title}</h2>}
            {activePage === 'flow' && screen === 'other' && <EmptyState title={phaseLabel(connection.state?.gameflowPhase)}
              description="Esta fase ainda não tem tela no easylobby. Use “Abrir client oficial”, no alto da janela, para continuar." />}
            {/* Posições estáveis: navegar ou mudar de tela não recria comandos em andamento. */}
            {/* Tela unificada (#129): o lobby fica sempre à vista; a busca e a partida encontrada aparecem embaixo dele. */}
            <div hidden={!playable || (activePage !== 'flow' && activePage !== 'home')} className={homeLayout ? 'home-primary'
              : lobbyMain ? 'grid min-h-0 flex-1 items-stretch gap-5 @min-[44rem]:grid-cols-[minmax(0,2fr)_minmax(16rem,1fr)]'
              : 'flex flex-1 flex-col gap-4'}>
              <LobbyPanel connection={connection} send={connection.send} />
              {/* Coluna da direita com largura; embaixo, presa à borda, quando a janela é estreita. */}
              <div hidden={activePage !== 'flow' || screen === 'home' || found} className="sticky bottom-0 z-10 h-full @min-[44rem]:top-0 @min-[44rem]:bottom-auto">
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
            <div hidden={activePage !== 'flow' || screen !== 'champSelect'}>
              <ChampSelectPanel connection={connection} send={connection.send}
                onOpenRunes={() => setNavigation({ screen, page: 'runes' })} />
            </div>
            {/* O editor de runas só abre pelo "Editar runas" da seleção, e só fica montado nela. */}
            {screen === 'champSelect' && <div hidden={activePage !== 'runes'} className="flex flex-col gap-4">
              {/* A volta fica à mão: a seleção continua montada e o timer segue correndo (#131). */}
              <button ref={runesBackRef} type="button" className="nav-button self-start"
                onClick={() => setNavigation({ screen, page: 'flow' })}>Voltar para {title}</button>
              <RunePagesPanel connection={connection} send={connection.send} />
            </div>}
          </div>
          </main>
        </div>
        <div hidden={!ready} className={sidebar.open ? 'app-social-region' : 'app-social-region is-collapsed'}>
          {!sidebar.open && <SidebarToggle open={false} onToggle={sidebar.toggle} invitations={invitations} unread={unread} />}
          {/* Sempre montada: escondida, a coluna mantém conversas e convites em andamento. */}
          <aside id="coluna-amigos" aria-label="Amigos e conversas" hidden={!sidebar.open} className="app-sidebar">
            {sidebar.open && <SidebarToggle open onToggle={sidebar.toggle} invitations={invitations} unread={unread} />}
            {/* Convites recebidos e o "Convidar" de cada amigo ficam aqui, sem seção própria. */}
            <InvitationsPanel connection={connection} send={connection.send}>
              {inviteAction => <FriendsList connection={connection} renderActions={friend => (
                <>{inviteAction(friend)}<ChatButton friend={friend} onOpen={setChatWith} /></>
              )} />}
            </InvitationsPanel>
            <ChatPanel connection={connection} send={connection.send} activeId={chatWith} onActiveChange={setChatWith} />
          </aside>
        </div>
      </div>
      <Notifications connection={connection} quiet={screen === 'champSelect'} sidebarOpen={sidebar.open} />
    </div>
  );
}
