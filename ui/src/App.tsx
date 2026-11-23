import { useEffect, useRef, useState } from 'react';
import { mainScreen, phaseLabel } from './screens/mainScreen';
import { liveGamePhases } from './agent/liveGame';
import { LiveGamePanel } from './components/LiveGamePanel';
import { LobbyPanel } from './components/LobbyPanel';
import { LobbyChatPanel } from './components/LobbyChatPanel';
import { LobbyGroupCard } from './components/LobbyInvites';
import { useAgent } from './agent/useAgent';
import { QueuePanel } from './components/QueuePanel';
import { RunesDialog } from './components/RunePagesPanel';
import { MatchFoundPending, ReadyCheckPanel } from './components/ReadyCheckPanel';
import { FriendsList } from './components/FriendsList';
import { InvitationsPanel } from './components/InvitationsPanel';
import { ChatButton, ConversationsPage } from './components/ConversationsPage';
import { Notifications } from './components/Notifications';
import { ChampSelectPanel } from './components/ChampSelectPanel';
import { PostGamePanel } from './components/PostGamePanel';
import { TitleBar } from './components/titlebar/TitleBar';
import { useTheme } from './theme/theme';
import { useSoundSettings } from './agent/soundSettings';
import { useAppSounds } from './agent/useAppSounds';
import { useAppUpdate } from './agent/useAppUpdate';
import { canInstallNow } from './agent/appUpdate';
import { useSidebar } from './layout/sidebar';
import { TopNavigation, ToolbarSettings, type NavigationPage } from './layout/TopNavigation';
import { SidebarPin } from './layout/SidebarPin';
import { SidebarToggle } from './layout/SidebarToggle';
import { connectionView } from './screens/connection';
import { ConnectionScreen } from './components/connection/ConnectionScreen';
import { SummonerHeader } from './components/home/SummonerHeader';
import { PatchNotesCard } from './components/home/PatchNotesCard';
import { FriendProfilePage, ProfilePage } from './components/profile/ProfilePage';
import { CollectionPage } from './components/collection/CollectionPage';
import { EmptyState } from './components/base';
import { totalUnread } from './agent/chat';
import { CompatibilityBanner, FeatureGate } from './components/CompatibilityNotice';
import { LcuSlowBanner } from './components/LcuSlowBanner';
import { VanguardBanner } from './components/VanguardNotice';
import { LolUpdateBanner } from './components/LolUpdateBanner';
import { unavailableFeature } from './agent/compatibility';

/** Runas não é destino da navegação: o editor só abre pela seleção de campeões (#165). */
type Page = NavigationPage | 'friendProfile';

export default function App() {
  const connection = useAgent();
  const { theme, setPreference } = useTheme();
  const soundSettings = useSoundSettings();
  const appUpdate = useAppUpdate();
  /** Última conversa aberta: o botão Conversas volta para ela (#251). */
  const [chatWith, setChatWith] = useState<string | null>(null);
  /**
   * Jogador cujo perfil está aberto (#255), os perfis abertos antes dele, por
   * um jogador do placar (#308), e a página para onde o último "Voltar" leva.
   */
  const [profileOf, setProfileOf] = useState<{ puuid: string; previous: string[]; back: 'main' | Page } | null>(null);
  const screen = mainScreen(connection);
  const view = connectionView(connection);
  const ready = view.screen === null;
  const sidebar = useSidebar();
  const invitations = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.receivedInvitations.length : 0;
  const unread = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? totalUnread(connection.state.conversations) : 0;
  const conversations = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.conversations : [];
  const unreadFrom = (id: string) => conversations.find(c => c.id === id)?.unreadCount ?? 0;
  const contentRef = useRef<HTMLElement>(null);
  const flowButtonRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (contentRef.current) contentRef.current.scrollTop = 0;
    if (document.activeElement?.closest('[hidden]')) flowButtonRef.current?.focus();
  }, [screen, ready]);
  const [runesOpen, setRunesOpen] = useState(false);
  // Trocar modo no lobby (#513): a tela de modos ocupa a área inteira, sem a busca e o grupo ao lado.
  const [changingMode, setChangingMode] = useState(false);
  // Lugar do botão Jogar, na barra do alto: no lobby, a busca de partida põe o botão dela ali (#600).
  const [queueSlot, setQueueSlot] = useState<HTMLDivElement | null>(null);
  // Sair da seleção fecha o pop-up de runas.
  if (runesOpen && screen !== 'champSelect') setRunesOpen(false);
  const [navigation, setNavigation] = useState<{ screen: typeof screen; page: 'main' | Page }>({ screen, page: 'main' });
  // Uma nova fase confirmada traz o fluxo principal de volta ao primeiro plano.
  if (navigation.screen !== screen) setNavigation({ screen, page: 'main' });
  // Vez do jogador de banir ou escolher (#431): volta para a seleção, de onde estiver no app.
  const turn = screen === 'champSelect' ? connection.state?.champSelect?.pendingAction?.id ?? null : null;
  const [lastTurn, setLastTurn] = useState<number | null>(turn);
  if (lastTurn !== turn) {
    setLastTurn(turn);
    if (turn !== null && navigation.page !== 'main' && navigation.page !== 'flow') setNavigation({ screen, page: 'main' });
  }
  const page = navigation.screen === screen ? navigation.page : 'main';
  const activePage: Page = page === 'main' ? (screen === 'home' ? 'home' : 'flow') : page;
  // Quem trocou de página por um botão que agora ficou escondido não perde o foco.
  useEffect(() => {
    if (!document.activeElement?.closest('[hidden]')) return;
    flowButtonRef.current?.focus();
  }, [activePage]);
  const openConversation = (id: string) => {
    setChatWith(id);
    setNavigation({ screen, page: 'conversations' });
  };
  const summoner = connection.status === 'connected' && connection.state?.lcu.state === 'connected'
    ? connection.state.summoner : null;
  const openPlayerProfile = (puuid: string) => {
    // O jogador logado tem a própria página de perfil.
    if (puuid === summoner?.puuid) {
      setNavigation({ screen, page: 'profile' });
      return;
    }
    if (page === 'friendProfile' && profileOf) {
      if (profileOf.puuid === puuid) return;
      setProfileOf({ puuid, previous: [...profileOf.previous, profileOf.puuid], back: profileOf.back });
    } else {
      setProfileOf({ puuid, previous: [], back: page });
    }
    setNavigation({ screen, page: 'friendProfile' });
  };
  const closePlayerProfile = () => {
    if (!profileOf) return;
    const previous = profileOf.previous.at(-1);
    if (previous) setProfileOf({ ...profileOf, puuid: previous, previous: profileOf.previous.slice(0, -1) });
    else setNavigation({ screen, page: profileOf.back });
  };
  const found = screen === 'found';
  // Início (#128) e lobby (#129) no layout de referência: cartões lado a lado quando há largura.
  const playable = screen === 'home' || screen === 'lobby' || screen === 'queue' || found;
  const homeLayout = activePage === 'home' && playable;
  useAppSounds(connection, ready && activePage === 'conversations' ? chatWith : null);
  // O cartão principal do Início ainda não tem função definida: criar lobby fica só no Jogar.
  // Com lobby, o Início continua igual (#611): o lobby fica só no Jogar, e o botão do alto leva até ele.
  const homeReserved = homeLayout;
  // A partida em andamento mostra as duas equipes (#268).
  const liveGame = screen === 'other' && liveGamePhases.has(connection.state?.gameflowPhase ?? '') ? connection.state?.liveGame ?? null : null;
  const lobbyMain = screen === 'lobby' || screen === 'queue';
  const lobbyState = connection.status === 'connected' && connection.state?.lcu.state === 'connected' ? connection.state.lobby : null;
  // Palco do lobby (#600): a personalizada mantém a coluna da direita com a busca e o grupo.
  const stage = !lobbyState?.custom;
  // Na tela do lobby, o botão do alto busca a partida (#600); nas outras páginas, continua levando ao lobby.
  // Lobby de altura fixa (#603): cabe na janela, sem rolagem; só o chat rola por dentro.
  const fixedLobby = ready && activePage === 'flow' && lobbyMain && stage && !changingMode;
  // Seleção de campeões de altura e largura fixas (#623): cabe na janela, sem rolagem; só a grade e o chat rolam por dentro.
  const fixedChampSelect = ready && activePage === 'flow' && screen === 'champSelect';
  const fixedScreen = fixedLobby || fixedChampSelect;
  const queueInSlot = ready && activePage === 'flow' && lobbyMain && stage && !!lobbyState && !changingMode
    && !unavailableFeature(connection, 'lobby');
  const title = screen === 'lobby' ? 'Lobby' : screen === 'queue' ? 'Fila'
    : found ? 'Partida encontrada' : screen === 'champSelect' ? 'Seleção de campeões'
      : screen === 'postGame' ? 'Fim de partida'
      : screen === 'other' ? (liveGame ? 'Partida em andamento' : 'Partida') : 'Início';
  return (
    // Margens e espaços vazios também arrastam a janela (#299). Sem valor, o
    // atributo só vale para cliques no próprio elemento, não nos filhos.
    <div data-tauri-drag-region className="app-shell">
      <TitleBar connection={connection} send={connection.send}
        navigation={<div hidden={!ready} className="contents">
          <TopNavigation active={activePage === 'friendProfile' ? 'profile' : activePage} flowLabel={screen === 'home' ? 'Jogar' : 'Grupo'}
            flowButtonRef={flowButtonRef} unread={unread} onNavigate={next => setNavigation({ screen, page: next })}
            queueSlotRef={setQueueSlot} queueInSlot={queueInSlot} />
        </div>}
        tools={<ToolbarSettings theme={theme} onThemeChange={setPreference}
          sound={soundSettings.settings} onSoundChange={soundSettings.update}
          update={appUpdate} canInstallUpdate={canInstallNow(connection.state?.lcu.state === 'connected' ? connection.state.gameflowPhase : null)} />} />
      <div data-tauri-drag-region className="app-body">
        <div data-tauri-drag-region className="app-main-column">
          <main ref={contentRef} className={fixedScreen ? 'app-content is-fixed' : 'app-content'}>
          {view.screen && <ConnectionScreen view={view.screen} send={connection.send} error={connection.error} />}
          {/* Escondido, e não desmontado, enquanto reconecta: comandos em andamento sobrevivem à queda. */}
          <div hidden={!ready} className={homeLayout ? 'home-grid' : activePage === 'conversations' || fixedScreen ? 'flex h-full min-h-0 flex-col gap-5' : 'flex min-h-full flex-col gap-5'}>
            {ready && connection.error && <p role="alert" className="col-span-full text-warn">{connection.error}</p>}
            {/* Modo de compatibilidade (#36): o que o LoL quebrou numa atualização. */}
            {/* Vanguard desconectado (#579): a partida não abriria e viraria esquiva. */}
            {ready && <VanguardBanner connection={connection} send={connection.send} />}
            {ready && <CompatibilityBanner connection={connection} send={connection.send} />}
            {/* O LoL demorando para responder ao conectar (#580). */}
            {ready && <LcuSlowBanner connection={connection} send={connection.send} />}
            {/* Patch do LoL em andamento (#575): progresso e fila travada. */}
            {ready && <LolUpdateBanner update={connection.state?.lolUpdate ?? null} />}
            <section hidden={activePage !== 'home'} aria-label="Início"
              className={homeLayout ? 'home-secondary' : 'flex max-w-[22rem] flex-col gap-4'}>
              {summoner && <SummonerHeader summoner={summoner} ranked={connection.state?.ranked ?? null} />}
              {homeLayout && <div className="home-placeholders">
                <PatchNotesCard notes={connection.state?.patchNotes ?? null} send={connection.send} />
                <article aria-label="Sem conteúdo" className="flex min-h-32 flex-col justify-end rounded-panel surface card p-5 shadow-soft animate-fade-up" style={{ animationDelay: '200ms' }}>
                  <h2 className="text-section font-semibold text-ink">Sem conteúdo</h2>
                  <p className="mt-1 text-body text-muted">Espaço reservado no momento.</p>
                </article>
              </div>}
              {screen !== 'home' && !playable && <button type="button" className="nav-button self-start" onClick={() => setNavigation({ screen, page: 'flow' })}>Voltar para {title}</button>}
            </section>
            {activePage === 'profile' && <section aria-label="Perfil"><FeatureGate connection={connection} send={connection.send} feature="profile">
              <ProfilePage connection={connection} send={connection.send} onOpenProfile={openPlayerProfile} />
            </FeatureGate></section>}
            {activePage === 'collection' && <section aria-label="Coleção"><CollectionPage connection={connection} send={connection.send} /></section>}
            {activePage === 'friendProfile' && profileOf && <section aria-label="Perfil do jogador">
              <FeatureGate connection={connection} send={connection.send} feature="profile">
                <FriendProfilePage key={profileOf.puuid} connection={connection} send={connection.send} puuid={profileOf.puuid}
                  onBack={closePlayerProfile} onChat={openConversation} onOpenProfile={openPlayerProfile} />
              </FeatureGate>
            </section>}
            {/* Sempre montada: o rascunho e a conversa escolhida sobrevivem à navegação. */}
            <section aria-label="Conversas" hidden={activePage !== 'conversations'} className="min-h-0 flex-1">
              <FeatureGate connection={connection} send={connection.send} feature="social">
                <ConversationsPage connection={connection} send={connection.send} activeId={chatWith} onActiveChange={setChatWith}
                  visible={ready && activePage === 'conversations'} />
              </FeatureGate>
            </section>
            {/* Na seleção (#623), o card da etapa faz as vezes de título: o nome da tela fica só para o leitor de tela. */}
            {activePage === 'flow' && screen === 'champSelect' && <h2 className="sr-only">{title}</h2>}
            {activePage === 'flow' && screen === 'other' && (liveGame && connection.state
              ? <LiveGamePanel state={connection.state} game={liveGame} onOpenProfile={openPlayerProfile} />
              : <EmptyState title={phaseLabel(connection.state?.gameflowPhase)}
                description="Esta fase ainda não tem tela no easylobby. Use “Abrir client oficial”, no alto da janela, para continuar." />)}
            {/* Montado durante todo o pós-jogo: o perfil aberto pelo placar (#585) não desfaz a honra votada. */}
            {screen === 'postGame' && <div hidden={activePage !== 'flow'} className="contents">
              <FeatureGate connection={connection} send={connection.send} feature="postGame">
                <PostGamePanel connection={connection} send={connection.send} onOpenProfile={openPlayerProfile} visible={activePage === 'flow'} />
              </FeatureGate>
            </div>}
            {/* Posições estáveis: navegar ou mudar de tela não recria comandos em andamento. */}
            {/* Tela unificada (#129): o lobby fica sempre à vista; a busca e a partida encontrada aparecem embaixo dele. */}
            <div hidden={!playable || (activePage !== 'flow' && activePage !== 'home')} className={homeLayout ? 'home-primary'
              : lobbyMain && !changingMode && stage ? 'flex min-h-0 flex-1 flex-col gap-5'
              : lobbyMain && !changingMode ? 'grid min-h-0 flex-1 items-stretch gap-5 @min-[44rem]:grid-cols-[minmax(0,2fr)_minmax(16rem,1fr)]'
              : 'flex flex-1 flex-col gap-4'}>
              {homeReserved && <article aria-label="Área reservada para atualização"
                className="flex h-full min-h-[24rem] flex-col items-start justify-end gap-3 rounded-panel surface card p-8 shadow-soft animate-fade-up @min-[44rem]:p-10">
                <span className="rounded-full bg-raised px-3 py-1 text-caption font-semibold uppercase tracking-wider text-muted">Em breve</span>
                <h2 className="text-display font-semibold text-ink">Área reservada</h2>
                <p className="max-w-md text-section text-muted">Este espaço vai ganhar uma nova função numa futura atualização. Para jogar, use o botão “Jogar” no alto da janela.</p>
              </article>}
              <div hidden={homeReserved} className="contents">
                <FeatureGate connection={connection} send={connection.send} feature="lobby">
                  <LobbyPanel connection={connection} send={connection.send} onOpenProfile={openPlayerProfile} onModeSelectChange={setChangingMode} />
                </FeatureGate>
              </div>
              {/* Personalizada: coluna da direita com largura, só da altura do conteúdo (sem sobra); embaixo, presa à borda, quando a janela é estreita.
                  Palco (#600): embaixo do lobby, o chat à esquerda e o estado da busca e o grupo à direita. */}
              <div hidden={activePage !== 'flow' || screen === 'home' || found || changingMode}
                className={stage ? 'grid min-h-0 flex-1 items-stretch gap-4 @min-[44rem]:grid-cols-[minmax(0,1fr)_minmax(16rem,22rem)]'
                  : 'sticky bottom-0 z-10 flex h-fit flex-col gap-4 self-start @min-[44rem]:top-0 @min-[44rem]:bottom-auto'}>
                {/* Com o lobby desligado (#36), o aviso fica no lugar do lobby; aqui não aparece nada. */}
                <FeatureGate connection={connection} send={connection.send} feature="lobby" quiet>
                  {stage ? <>
                    <div className="flex min-h-0 min-w-0 flex-col gap-4">
                      <LobbyChatPanel connection={connection} send={connection.send} fill />
                    </div>
                    <div className="flex min-h-0 min-w-0 flex-col gap-4 overflow-y-auto">
                      <QueuePanel connection={connection} send={connection.send} buttonSlot={queueInSlot ? queueSlot : null} />
                      <LobbyGroupCard connection={connection} />
                    </div>
                  </> : <>
                    <QueuePanel connection={connection} send={connection.send} />
                    {/* Embaixo da busca, quem foi convidado e o status, e o chat do grupo (#206). */}
                    <LobbyGroupCard connection={connection} />
                    <LobbyChatPanel connection={connection} send={connection.send} />
                  </>}
                </FeatureGate>
              </div>
            </div>
            {/* A partida encontrada abre num pop-up no meio da tela, por cima de tudo, em qualquer página (#649). */}
            {found && !connection.state?.readyCheck && <MatchFoundPending />}
            {found && <FeatureGate connection={connection} send={connection.send} feature="readyCheck">
              <ReadyCheckPanel connection={connection} send={connection.send} />
            </FeatureGate>}
            {/* Montada fora da tela também, para a busca e as ações em andamento sobreviverem à navegação. */}
            <div hidden={activePage !== 'flow' || screen !== 'champSelect'} className="flex min-h-0 flex-1 flex-col">
              <FeatureGate connection={connection} send={connection.send} feature="champSelect">
                <ChampSelectPanel connection={connection} send={connection.send}
                  onOpenRunes={() => setRunesOpen(true)} />
              </FeatureGate>
            </div>
            {/* O editor de runas abre num pop-up por cima da seleção (#429), e só existe nela. */}
            {screen === 'champSelect' && runesOpen && !unavailableFeature(connection, 'runes')
              && <RunesDialog connection={connection} send={connection.send} onClose={() => setRunesOpen(false)} />}
          </div>
          </main>
        </div>
        {/* Aba de amigos (#594): fechada numa faixa fina; com o mouse em cima, a própria faixa cresce por cima do
            conteúdo até a largura da aba, sem empurrá-lo. Fixada (#643), fica aberta ao lado do conteúdo e o empurra. */}
        <div hidden={!ready} {...sidebar.regionProps}
          className={['app-social-region', sidebar.open && 'is-open', sidebar.pinned && 'is-pinned', sidebar.closing && 'is-closing', sidebar.dragging && 'is-dragging'].filter(Boolean).join(' ')}>
          <div className="app-social-rail">
            <SidebarToggle open={false} onToggle={sidebar.toggle} invitations={invitations} unread={unread} />
          {/* Sempre montada: escondida, a coluna mantém conversas e convites em andamento.
              Durante o arraste de um amigo ela só sai da vista (CSS), para não cancelar o arraste. */}
          <aside id="coluna-amigos" aria-label="Amigos e convites" hidden={!sidebar.open && !sidebar.closing && !sidebar.dragging} className="app-sidebar">
            {sidebar.open && <div className="app-sidebar-actions">
              <SidebarPin pinned={sidebar.pinned} onToggle={sidebar.togglePinned} />
              <SidebarToggle open onToggle={sidebar.toggle} invitations={invitations} unread={unread} />
            </div>}
            {/* Convites recebidos ficam aqui, sem seção própria; convidar é pelo lobby (#206). */}
            <FeatureGate connection={connection} send={connection.send} feature="lobby" quiet>
              <InvitationsPanel connection={connection} send={connection.send} />
            </FeatureGate>
            <FeatureGate connection={connection} send={connection.send} feature="social">
              <FriendsList connection={connection} onOpenProfile={friend => openPlayerProfile(friend.puuid)} renderActions={friend => (
                // A área inteira à direita da linha abre a conversa, não só o ícone (#255).
                <ChatButton friend={friend} unread={unreadFrom(friend.id)} onOpen={openConversation}
                  className="h-auto! w-14 rounded-control! hover:bg-line/40!" />
              )} />
            </FeatureGate>
          </aside>
          </div>
        </div>
      </div>
      <Notifications connection={connection} quiet={screen === 'champSelect'} sidebarOpen={sidebar.open}
        openConversationId={ready && activePage === 'conversations' ? chatWith : null} onOpenConversation={openConversation} />
    </div>
  );
}
