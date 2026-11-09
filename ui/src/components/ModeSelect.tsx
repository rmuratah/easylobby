import { useId, useState, type ReactNode } from 'react';
import { blockCurrentQueue, buildModeTabs, firstAvailable, type CreateLobby, type GameMode, type ModeTab, type ModeTabId, type Submode, type TrainingCommand } from '../agent/gameModes';
import type { GameQueue } from '../bindings/GameQueue';
import type { TutorialPart } from '../bindings/TutorialPart';
import { defaultTutorialPart, tutorialCards } from '../agent/training';
import { TutorialParts } from './TutorialParts';
import { cx } from './base/cx';
import { CustomGameForm } from './CustomGameForm';
import { customOptionsProblem, customOptionsSummary, defaultCustomOptions } from '../agent/customGame';
import type { CustomGameOptions } from '../bindings/CustomGameOptions';
import type { CustomGameListing } from '../bindings/CustomGameListing';
import type { AgentClient } from '../agent/client';
import { CustomGameBrowser } from './CustomGameBrowser';
import { Segmented } from './base/Segmented';
import { listingMapName, listingModeName } from '../agent/customGame';

type Props = {
  queues: GameQueue[];
  /** Pode criar lobby agora (logado, fora da busca e da partida). */
  canCreate: boolean;
  pending: boolean;
  /** Texto de ajuda embaixo do botão (o porquê de estar bloqueado, por exemplo). */
  restriction: string;
  descriptionId: string;
  feedback: ReactNode;
  onCreate: (command: CreateLobby) => void;
  onStart: (command: TrainingCommand) => void;
  /** Progresso do Tutorial (#228), para os cards das partes. */
  tutorial?: TutorialPart[] | null;
  /**
   * Já em lobby (#231): a tela troca a fila em vez de criar o lobby. Abre na
   * aba da fila atual, sem a aba Treinamento, e tem um botão para voltar ao
   * lobby. A fila atual fica bloqueada (#239).
   */
  lobby?: { queueId: number; queueName: string; onBack: () => void };
  /** Nome do invocador, para o nome sugerido da personalizada ("Partida de ..."). */
  summonerName?: string;
  /** Cria a partida personalizada da aba Personalizada (#262). */
  onCreateCustom?: (options: CustomGameOptions) => void;
  /**
   * "Entrar" da aba Personalizada (#264): a lista de personalizadas abertas
   * (`null` antes da primeira busca), o canal para atualizar a lista e entrar
   * numa delas com a senha (vazia sem senha). Sem isso, a aba só cria.
   */
  customJoin?: {
    games: CustomGameListing[] | null;
    send: AgentClient['send'];
    onJoin: (id: number, password: string) => void;
  };
};
type CustomView = 'create' | 'join';
type Selection = { tab: ModeTabId; mode: GameMode['id'] | null; queueId: number | null };

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const primaryPill = `inline-flex h-11 shrink-0 items-center justify-center gap-2 rounded-full px-6 text-body font-semibold transition-all disabled:cursor-not-allowed bg-accent text-on-accent shadow-glow enabled:hover:-translate-y-0.5 disabled:bg-raised disabled:text-muted disabled:shadow-none ${focusRing}`;

/** Ícone do modo (#294); sem imagem, a sigla. Modo fechado fica acinzentado. */
function ModeIcon({ mode, size = 'md' }: { mode: GameMode; size?: 'md' | 'lg' }) {
  const box = size === 'lg' ? 'size-14' : 'size-11';
  if (mode.image) {
    return (
      <img src={mode.image} alt="" aria-hidden="true" draggable={false}
        className={cx('shrink-0 object-contain', box, !mode.available && 'opacity-50 grayscale')} />
    );
  }
  return (
    <span aria-hidden="true" className={cx('flex shrink-0 items-center justify-center rounded-tile font-semibold tracking-wide',
      mode.available ? 'bg-primary-gradient text-on-accent' : 'bg-raised text-muted',
      size === 'lg' ? 'size-14 text-section' : 'size-11 text-body')}>
      {mode.icon}
    </span>
  );
}

/**
 * Tela de escolha de modo de jogo (#210), no lugar do card "Pronto para
 * jogar?" quando o jogador não está em lobby: abas PvP, Co-op versus IA e
 * Treinamento; cards de modo com os submodos em botões de rádio; rodapé com a
 * fila escolhida e o botão de criar o lobby (ou de iniciar, no Treinamento).
 */
/**
 * Seleção inicial da troca de fila: a aba da fila atual, com outra fila aberta
 * do mesmo modo marcada (a atual está bloqueada, #239). Sem outra aberta no
 * modo, fica a primeira aberta da aba.
 */
function selectionOf(tabs: ModeTab[], queueId: number | undefined): Selection {
  for (const tab of tabs) {
    const mode = tab.modes.find(m => m.submodes.some(s => s.queueId === queueId));
    if (!mode) continue;
    const other = mode.submodes.find(s => s.available);
    return other ? { tab: tab.id, mode: mode.id, queueId: other.queueId } : { tab: tab.id, mode: null, queueId: null };
  }
  return { tab: 'pvp', mode: null, queueId: null };
}

const currentQueueText = 'Você já está nesta fila.';

export function ModeSelect({ queues, canCreate, pending, restriction, descriptionId, feedback, onCreate, onStart, tutorial = null, lobby, summonerName, onCreateCustom, customJoin }: Props) {
  const allTabs = buildModeTabs(queues);
  // Tutorial, Modo de Treino e personalizada não trocam a fila de um lobby.
  // A fila atual do lobby não pode ser escolhida (#239).
  const tabs = lobby ? blockCurrentQueue(allTabs.filter(t => t.id !== 'training' && t.id !== 'custom'), lobby.queueId)
    : allTabs.filter(t => t.id !== 'custom' || onCreateCustom);
  const baseId = useId();
  const [selection, setSelection] = useState<Selection>(() => selectionOf(tabs, lobby?.queueId));
  const tab = tabs.find(t => t.id === selection.tab) ?? tabs[0];

  // A escolha vale enquanto o modo (e o submodo) continuarem abertos; senão, o primeiro aberto da aba.
  const chosenMode = tab.modes.find(m => m.id === selection.mode && m.available);
  const chosenSubmode = chosenMode?.submodes.find(s => s.queueId === selection.queueId && s.available) ?? null;
  const current = chosenMode && (chosenSubmode || !chosenMode.submodes.length)
    ? { mode: chosenMode, submode: chosenSubmode }
    : firstAvailable(tab);
  const locked = !canCreate || pending;
  const isTraining = tab.id === 'training';
  const isCustom = tab.id === 'custom';
  // Formulário da personalizada (#262): fica preenchido ao trocar de aba e voltar.
  const [customOptions, setCustomOptions] = useState<CustomGameOptions>(() => defaultCustomOptions(summonerName));
  // O invocador pode chegar depois de a tela montar (#278): o nome sugerido
  // acompanha, a menos que o jogador já tenha editado o campo.
  const [suggestedFor, setSuggestedFor] = useState(summonerName);
  if (suggestedFor !== summonerName) {
    setSuggestedFor(summonerName);
    if (customOptions.name === defaultCustomOptions(suggestedFor).name) {
      setCustomOptions({ ...customOptions, name: defaultCustomOptions(summonerName).name });
    }
  }
  const customProblem = customOptionsProblem(customOptions);
  // Criar ou entrar numa personalizada aberta (#264).
  const [customView, setCustomView] = useState<CustomView>('create');
  const joining = isCustom && !!customJoin && customView === 'join';
  const [joinChoice, setJoinChoice] = useState<number | null>(null);
  const [joinPassword, setJoinPassword] = useState('');
  // A partida escolhida some da tela se sair da lista numa atualização.
  const joinGame = customJoin?.games?.find(g => g.id === joinChoice) ?? null;
  const joinBlocked = !joinGame || (joinGame.hasPassword && !joinPassword);
  // Tutorial (#228): a parte escolhida nos cards; sem escolha, a primeira liberada que falta.
  const [tutorialChoice, setTutorialChoice] = useState<number | null>(null);
  const parts = tutorialCards(tutorial);
  const choice = parts.find(p => p.queueId === tutorialChoice && p.available);
  const tutorialPart = choice ?? parts.find(p => p.queueId === defaultTutorialPart(parts))!;
  const isTutorial = current?.mode.id === 'tutorial';
  // A fila atual já vem bloqueada; a guarda fica por segurança.
  const unchanged = !!lobby && current?.submode?.queueId === lobby.queueId;

  function chooseTab(id: ModeTabId) {
    setSelection({ tab: id, mode: null, queueId: null });
  }
  function choose(mode: GameMode, submode: Submode | null) {
    setSelection({ tab: tab.id, mode: mode.id, queueId: submode?.queueId ?? null });
  }
  function chooseGame(id: number) {
    if (id !== joinChoice) setJoinPassword('');
    setJoinChoice(id);
  }
  function confirm() {
    if (joining) {
      if (joinGame && !joinBlocked) customJoin?.onJoin(joinGame.id, joinGame.hasPassword ? joinPassword : '');
      return;
    }
    if (isCustom) {
      if (!customProblem) onCreateCustom?.({ ...customOptions, name: customOptions.name.trim() });
      return;
    }
    if (!current) return;
    if (isTutorial) onStart({ type: 'startTutorial', queueId: tutorialPart.queueId });
    else if (current.mode.start) onStart(current.mode.start);
    else if (current.submode) onCreate(current.submode.command);
  }

  const radioName = `${baseId}-${tab.id}`;
  return (
    <section aria-label="Lobby" className="flex h-full min-h-[24rem] flex-col gap-5 rounded-panel bg-surface p-6 text-ink shadow-lift animate-fade-up @min-[44rem]:p-8">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <span className="text-caption font-semibold uppercase tracking-wider text-accent-ink">{lobby ? 'Lobby' : 'Jogar'}</span>
          <h2 className="text-title font-semibold">{lobby ? 'Trocar modo de jogo' : 'Escolha o modo de jogo'}</h2>
          <p className="text-body text-muted">{lobby ? `Você está no lobby de ${lobby.queueName}.` : 'Você não está em um lobby.'}</p>
        </div>
        <div role="tablist" aria-label="Tipo de partida" className="inline-flex gap-0.5 rounded-control bg-raised p-[3px]">
          {tabs.map(t => (
            <button key={t.id} type="button" role="tab" id={`${baseId}-tab-${t.id}`} aria-selected={t.id === tab.id}
              aria-controls={`${baseId}-panel`} onClick={() => chooseTab(t.id)}
              className={cx('inline-flex h-9 items-center rounded-control-sm px-4 text-body font-medium', focusRing,
                t.id === tab.id ? 'bg-surface text-ink shadow-[0_0_0_1px_var(--el-line)]' : 'text-muted hover:text-ink')}>
              {t.label}
            </button>
          ))}
        </div>
      </header>

      {isCustom ? (
        <div role="tabpanel" id={`${baseId}-panel`} aria-labelledby={`${baseId}-tab-${tab.id}`} className="flex-1">
          {customJoin && (
            <Segmented label="Criar ou entrar" className="mb-5" value={customView} onChange={setCustomView}
              options={[{ value: 'create', label: 'Criar' }, { value: 'join', label: 'Entrar' }]} />
          )}
          {joining && customJoin ? (
            <CustomGameBrowser games={customJoin.games} selectedId={joinGame?.id ?? null} onSelect={chooseGame}
              locked={locked} send={customJoin.send} />
          ) : (
            <CustomGameForm options={customOptions} onChange={setCustomOptions} locked={locked} problem={customProblem} />
          )}
        </div>
      ) : (
      <div role="tabpanel" id={`${baseId}-panel`} aria-labelledby={`${baseId}-tab-${tab.id}`}
        className={cx('grid flex-1 content-start gap-4',
          isTraining ? '@min-[40rem]:grid-cols-2'
            : tab.modes.length > 1 && '@min-[40rem]:grid-cols-2 @min-[64rem]:grid-cols-[repeat(auto-fit,minmax(14rem,1fr))]')}>
        {tab.modes.map(mode => {
          const selected = current?.mode.id === mode.id;
          if (!mode.submodes.length) {
            // Treinamento: o card inteiro é a escolha.
            return (
              <label key={mode.id} className={cx('flex min-h-40 cursor-pointer flex-col gap-3 rounded-panel p-5 transition-all',
                selected ? 'bg-accent-soft ring-2 ring-accent' : 'surface shadow-soft hover:-translate-y-0.5 hover:shadow-lift',
                locked && 'cursor-not-allowed')}>
                <input type="radio" name={radioName} className="sr-only" checked={selected} disabled={locked}
                  onChange={() => choose(mode, null)} aria-describedby={`${baseId}-${mode.id}-desc`} />
                <ModeIcon mode={mode} size="lg" />
                <span className="text-section font-semibold">{mode.name}</span>
                <span id={`${baseId}-${mode.id}-desc`} className="text-body text-muted">{mode.description}</span>
              </label>
            );
          }
          return (
            <fieldset key={mode.id} aria-label={mode.name} disabled={!mode.available}
              className={cx('flex min-w-0 flex-col gap-3 rounded-panel p-4 transition-all',
                !mode.available ? 'bg-raised/60 opacity-60'
                  : selected ? 'bg-accent-soft ring-2 ring-accent' : 'surface shadow-soft')}>
              <div className="flex items-start gap-3">
                <ModeIcon mode={mode} />
                <div className="min-w-0">
                  <h3 className="text-section font-semibold">{mode.name}</h3>
                  <p className="text-caption text-muted">{mode.available ? mode.description
                    : mode.submodes.some(s => s.current) ? currentQueueText : 'Fechado agora.'}</p>
                </div>
              </div>
              <div className={cx('grid gap-1.5', tab.modes.length === 1 && '@min-[40rem]:grid-cols-3')}>
                {mode.submodes.map(submode => {
                  const checked = selected && current?.submode?.queueId === submode.queueId;
                  return (
                    <label key={submode.queueId} className={cx('flex items-start gap-2.5 rounded-tile px-3 py-2 transition-colors',
                      !submode.available || locked ? 'cursor-not-allowed' : 'cursor-pointer hover:bg-raised',
                      !submode.available && 'opacity-60', checked && 'bg-surface')}
                      title={submode.current ? currentQueueText : undefined}>
                      <input type="radio" name={radioName} value={submode.queueId} checked={checked}
                        disabled={!submode.available || locked} onChange={() => choose(mode, submode)}
                        className="mt-1 size-4 shrink-0 accent-[var(--el-accent)]" />
                      <span className="min-w-0">
                        <span className="block text-body font-semibold">{submode.name}</span>
                        <span className="block text-caption text-muted">{submode.available ? submode.description
                          : submode.current ? currentQueueText : 'Fechada agora.'}</span>
                      </span>
                    </label>
                  );
                })}
              </div>
            </fieldset>
          );
        })}
      </div>
      )}

      {isTutorial && (
        <TutorialParts cards={parts} selected={tutorialPart.queueId} locked={locked} name={`${baseId}-tutorial`}
          onSelect={setTutorialChoice} />
      )}

      <footer className="flex flex-wrap items-center gap-4 rounded-panel surface p-4 shadow-soft">
        {joining ? (
          <div className="flex min-w-0 flex-1 flex-wrap items-center gap-3">
            <div className="min-w-[12rem] flex-1">
              <p className="text-caption font-semibold uppercase tracking-wider text-muted">Entrar na personalizada</p>
              <p className="truncate text-body font-semibold" aria-live="polite">{joinGame ? joinGame.name || 'Sem nome' : 'Escolha uma partida da lista'}</p>
              {joinGame && (
                <p className="truncate text-caption text-muted">
                  {[joinGame.owner, listingMapName(joinGame), listingModeName(joinGame), `${joinGame.players}/${joinGame.maxPlayers} jogadores`].filter(Boolean).join(' · ')}
                </p>
              )}
            </div>
            {joinGame?.hasPassword && (
              <div className="flex w-48 max-w-full flex-col gap-1">
                <label htmlFor={`${baseId}-join-password`} className="text-caption font-semibold uppercase tracking-wider text-muted">Senha</label>
                <input id={`${baseId}-join-password`} type="password" value={joinPassword} disabled={locked} autoComplete="off"
                  onChange={event => setJoinPassword(event.target.value)}
                  onKeyDown={event => { if (event.key === 'Enter') confirm(); }}
                  className={`h-10 w-full min-w-0 rounded-control border border-control bg-raised px-3 text-body text-ink disabled:cursor-not-allowed ${focusRing}`} />
              </div>
            )}
          </div>
        ) : isCustom ? (
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <span aria-hidden="true" className="flex size-11 shrink-0 items-center justify-center rounded-tile bg-primary-gradient text-body font-semibold tracking-wide text-on-accent">PE</span>
            <div className="min-w-0">
              <p className="text-caption font-semibold uppercase tracking-wider text-muted">Partida personalizada</p>
              <p className="truncate text-body font-semibold" aria-live="polite">{customOptions.name.trim() || 'Sem nome'}</p>
              <p className="truncate text-caption text-muted">
                {customOptionsSummary(customOptions)}{customOptions.password ? ' · Com senha' : ''}
              </p>
            </div>
          </div>
        ) : current ? (
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <ModeIcon mode={current.mode} />
            <div className="min-w-0">
              <p className="text-caption font-semibold uppercase tracking-wider text-muted">{isTraining ? 'Treino selecionado' : 'Fila selecionada'}</p>
              <p className="truncate text-body font-semibold" aria-live="polite">
                {current.submode ? `${current.mode.name} — ${current.submode.name}`
                  : isTutorial ? `${current.mode.name} — Parte ${tutorialPart.part}` : current.mode.name}
              </p>
              <p className="truncate text-caption text-muted">{current.submode?.description ?? (isTutorial ? tutorialPart.name : current.mode.description)}</p>
            </div>
          </div>
        ) : <p className="min-w-0 flex-1 text-body text-muted">Nenhum modo aberto nesta aba agora.</p>}
        {lobby && (
          <button type="button" onClick={lobby.onBack}
            className={`inline-flex h-11 shrink-0 items-center justify-center rounded-full px-5 text-body font-semibold text-muted transition-colors hover:text-ink ${focusRing}`}>
            Voltar ao lobby
          </button>
        )}
        <button type="button" className={primaryPill} aria-describedby={descriptionId} aria-busy={pending}
          disabled={joining ? joinBlocked || locked : isCustom ? !!customProblem || locked : !current || locked || unchanged} onClick={confirm}>
          {joining ? 'Entrar' : isCustom ? 'Confirmar' : isTraining ? 'Iniciar' : lobby ? 'Trocar fila' : 'Criar lobby'}
        </button>
      </footer>
      <p id={descriptionId} className="text-caption text-muted">{restriction}</p>
      {feedback}
    </section>
  );
}
