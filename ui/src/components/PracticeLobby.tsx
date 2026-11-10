import { useEffect, useId, useRef, useState, type ReactNode } from 'react';
import type { AgentClient } from '../agent/client';
import { searchChampions } from '../agent/champSelect';
import { playerName } from '../agent/friends';
import { positionNames } from '../agent/positions';
import { assetUrl } from '../agent/assets';
import { useCommand } from '../agent/useCommand';
import { BOT_POSITIONS, botDifficultyNames } from '../agent/customLobby';
import { customLobbySummary, customLobbyTitle, spectatorNames } from '../agent/customGame';
import type { AgentState } from '../bindings/AgentState';
import type { BotDifficulty } from '../bindings/BotDifficulty';
import type { Champion } from '../bindings/Champion';
import type { CustomBot } from '../bindings/CustomBot';
import type { CustomMember } from '../bindings/CustomMember';
import type { CustomTeam } from '../bindings/CustomTeam';
import type { Lobby } from '../bindings/Lobby';
import type { Position } from '../bindings/Position';
import { Avatar } from './base';
import { cx } from './base/cx';
import { ChampionIcon } from './ChampSelectPanel';
import { CloseIcon, CrownIcon, PlusIcon } from './home/icons';
import { LobbyMemberMenu } from './LobbyMemberMenu';
import { LobbyOpenToggle } from './LobbyOpenToggle';
import { PositionIcon } from './PositionIcon';

type Props = {
  lobby: Lobby & { custom: NonNullable<Lobby['custom']> };
  state: AgentState;
  send: AgentClient['send'];
  /** Sair do lobby, pelo mesmo comando do lobby comum. */
  canLeave: boolean;
  leaving: boolean;
  onLeave: () => void;
  /** Linha de retorno do lobby (sair, por exemplo). */
  feedback: ReactNode;
};

const DIFFICULTIES = Object.keys(botDifficultyNames) as (keyof typeof botDifficultyNames)[];
const teamNames: Record<CustomTeam, string> = { team1: 'Equipe 1', team2: 'Equipe 2' };

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const selectClass = `h-9 min-w-0 rounded-control border border-control bg-raised px-2 text-caption text-ink disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;
const pill = `inline-flex h-9 items-center justify-center gap-1.5 rounded-full px-4 text-caption font-semibold transition-colors disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;

/** Quem escolhe o campeão: um bot novo numa equipe, ou um bot que já está no lobby. */
type Picking = { team: CustomTeam } | { bot: CustomBot };

/**
 * Lobby de personalizada, do Modo de Treino (#229) e da partida criada na aba
 * Personalizada (#262): Equipe 1 e Equipe 2 com as vagas, os bots (campeão,
 * posição, dificuldade e um X para tirar), "Juntar-se" na equipe do outro
 * lado, os espectadores com o botão Assistir (#263) e o botão de iniciar a
 * partida. O campeão do bot sai de um pop-up com busca e grade, no lugar da
 * lista suspensa do client.
 */
export function PracticeLobby({ lobby, state, send, canLeave, leaving, onLeave, feedback }: Props) {
  const { custom } = lobby;
  const me = state.summoner?.puuid ?? '';
  const myTeam: CustomTeam | null = custom.team1.some(m => m.puuid === me) ? 'team1'
    : custom.team2.some(m => m.puuid === me) ? 'team2' : null;
  const command = useCommand(send);
  const [picking, setPicking] = useState<Picking | null>(null);
  const phaseOk = state.gameflowPhase === 'Lobby' && !state.matchmaking && !state.readyCheck;
  const canEditBots = lobby.isLeader && phaseOk && !command.pending;
  const canStart = lobby.isLeader && phaseOk && !command.pending;
  const spectating = custom.spectators.some(m => m.puuid === me);
  const spectatorPolicy = custom.settings.spectators;
  // Sem saber a política, deixa tentar: o agente confere e a LCU decide.
  const canWatch = spectatorPolicy !== 'none' && !spectating && phaseOk && !command.pending
    && custom.spectators.length < custom.maxSpectators;
  const showSpectators = spectatorPolicy !== 'none' || custom.spectators.length > 0;

  function addBot(team: CustomTeam, championId: number) {
    const taken = new Set(custom[team].map(m => m.bot?.position));
    const position = BOT_POSITIONS.find(p => !taken.has(p)) ?? 'middle';
    void command.run({ type: 'addBot', team, championId, difficulty: 'intro', position },
      { pending: 'Adicionando o bot…', failure: 'adicionar o bot' });
  }
  function updateBot(bot: CustomBot, change: Partial<Pick<CustomBot, 'championId' | 'difficulty' | 'position'>>) {
    const next = { ...bot, ...change };
    const position = (BOT_POSITIONS as readonly Position[]).includes(next.position) ? next.position : 'middle';
    const difficulty = next.difficulty === 'unknown' ? 'intro' : next.difficulty;
    void command.run({ type: 'updateBot', botUuid: bot.uuid, championId: next.championId, difficulty, position },
      { pending: 'Mudando o bot…', failure: 'mudar o bot' });
  }

  return (
    <section aria-label="Lobby" className="flex h-full min-h-0 w-full flex-col gap-4 rounded-panel text-ink">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Lobby</h2>
          <p className="text-title font-semibold"><span className="sr-only">Fila atual: </span><strong className="font-semibold">{customLobbyTitle(custom.settings)}</strong></p>
          <p className="text-caption text-muted">{customLobbySummary(custom.settings, custom.teamSize)}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <span className="rounded-full surface px-3 py-1.5 shadow-soft text-caption font-medium text-muted">
            {lobby.isLeader ? 'Você é o líder' : 'Você é membro'}
          </span>
          <LobbyOpenToggle lobby={lobby} send={send} disabled={!phaseOk} />
          <button type="button" aria-label="Sair do lobby" title="Sair do lobby" aria-busy={leaving}
            disabled={!canLeave || leaving} onClick={onLeave}
            className={`flex size-10 items-center justify-center rounded-tile surface text-muted shadow-soft transition-colors enabled:hover:text-danger disabled:cursor-not-allowed ${focusRing}`}>
            <CloseIcon />
          </button>
        </div>
      </header>

      <div className="grid min-h-0 flex-1 gap-4 @min-[44rem]:grid-cols-2">
        {(['team1', 'team2'] as const).map(team => {
          const members = custom[team];
          const free = Math.max(0, custom.teamSize - members.length);
          return (
            <section key={team} aria-label={teamNames[team]} className="flex min-w-0 flex-col gap-2 rounded-panel surface card p-3 shadow-soft">
              <header className="flex items-center justify-between gap-2 px-1">
                <h3 className="text-section font-semibold">{teamNames[team]}</h3>
                <span className="text-caption text-muted">{members.length} / {custom.teamSize}</span>
              </header>
              <ul aria-label={`Vagas da ${teamNames[team]}`} className="flex flex-col gap-2">
                {members.map((member, index) => (
                  <li key={member.bot?.uuid ?? member.puuid ?? index}>
                    {member.bot
                      ? <BotRow bot={member.bot} state={state} editable={canEditBots}
                          onChampion={() => setPicking({ bot: member.bot! })}
                          onChange={change => updateBot(member.bot!, change)}
                          onRemove={() => { void command.run({ type: 'removeBot', botUuid: member.bot!.uuid }, { pending: 'Tirando o bot…', failure: 'tirar o bot' }); }} />
                      : <PlayerRow member={member} me={member.puuid === me} state={state} send={send} manage={lobby.isLeader} disabled={!phaseOk} />}
                  </li>
                ))}
                {Array.from({ length: free }, (_, index) => (
                  <li key={`vaga-${index}`} aria-label="Vaga livre"
                    className="flex min-h-14 flex-wrap items-center justify-center gap-2 rounded-tile border-2 border-dashed border-line px-3 py-2">
                    {lobby.isLeader && (
                      <button type="button" disabled={!canEditBots} aria-haspopup="dialog" onClick={() => setPicking({ team })}
                        className={cx(pill, 'bg-raised text-ink enabled:hover:bg-accent enabled:hover:text-on-accent')}>
                        <PlusIcon size={14} /> Adicionar bot
                      </button>
                    )}
                    {myTeam !== team && index === 0 && (
                      <button type="button" disabled={!phaseOk || command.pending}
                        onClick={() => { void command.run({ type: 'switchTeam', team }, { pending: 'Trocando de equipe…', failure: 'trocar de equipe' }); }}
                        className={cx(pill, 'surface text-accent-ink shadow-soft enabled:hover:bg-accent-soft')}>
                        Juntar-se
                      </button>
                    )}
                    {!lobby.isLeader && (myTeam === team || index > 0) && <span className="text-caption text-muted">Vaga livre</span>}
                  </li>
                ))}
              </ul>
            </section>
          );
        })}
      </div>

      {showSpectators && (
        <section aria-label="Espectadores" className="flex flex-col gap-2 rounded-panel surface card p-3 shadow-soft">
          <header className="flex flex-wrap items-center justify-between gap-2 px-1">
            <div className="min-w-0">
              <h3 className="text-section font-semibold">Espectadores</h3>
              {spectatorPolicy && <p className="text-caption text-muted">Quem pode assistir: {spectatorNames[spectatorPolicy]}</p>}
            </div>
            <div className="flex items-center gap-2">
              <span className="text-caption text-muted">{custom.spectators.length} / {custom.maxSpectators}</span>
              {!spectating && spectatorPolicy !== 'none' && (
                <button type="button" disabled={!canWatch}
                  onClick={() => { void command.run({ type: 'spectate' }, { pending: 'Indo para os espectadores…', failure: 'assistir' }); }}
                  className={cx(pill, 'surface text-accent-ink shadow-soft enabled:hover:bg-accent-soft')}>
                  Assistir
                </button>
              )}
            </div>
          </header>
          {custom.spectators.length > 0 ? (
            <ul aria-label="Jogadores assistindo" className="grid gap-2 @min-[44rem]:grid-cols-2">
              {custom.spectators.map((member, index) => (
                <li key={member.puuid || index}><PlayerRow member={member} me={member.puuid === me} state={state} send={send} manage={lobby.isLeader} disabled={!phaseOk} /></li>
              ))}
            </ul>
          ) : <p className="px-1 text-caption text-muted">Ninguém assistindo.</p>}
          {spectating && <p className="px-1 text-caption text-muted">Você está assistindo. Use Juntar-se para voltar a uma equipe.</p>}
        </section>
      )}

      <footer className="flex flex-wrap items-center justify-between gap-3 rounded-panel surface card p-4 shadow-soft">
        <p className="min-w-0 flex-1 text-caption text-muted">
          {lobby.isLeader ? 'Monte as equipes e inicie a partida.' : 'Aguarde o líder iniciar a partida.'}
        </p>
        {lobby.isLeader && (
          <button type="button" disabled={!canStart} aria-busy={command.pending}
            onClick={() => { void command.run({ type: 'startCustomGame' }, { pending: 'Iniciando a partida…', failure: 'iniciar a partida' }); }}
            className={`inline-flex h-11 items-center justify-center rounded-full bg-accent px-6 text-body font-semibold text-on-accent shadow-glow transition-all enabled:hover:-translate-y-0.5 disabled:cursor-not-allowed disabled:bg-raised disabled:text-muted disabled:shadow-none ${focusRing}`}>
            Iniciar partida
          </button>
        )}
      </footer>
      {command.feedback.message && <p role={command.feedback.status === 'error' ? 'alert' : 'status'}
        className={command.feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{command.feedback.message}</p>}
      {feedback}

      {picking && (
        <ChampionPicker state={state} current={'bot' in picking ? picking.bot.championId : null}
          title={'bot' in picking ? 'Campeão do bot' : `Novo bot na ${teamNames[picking.team]}`}
          onClose={() => setPicking(null)}
          onChoose={championId => {
            setPicking(null);
            if ('bot' in picking) {
              if (championId !== picking.bot.championId) updateBot(picking.bot, { championId });
            } else addBot(picking.team, championId);
          }} />
      )}
    </section>
  );
}

function PlayerRow({ member, me, state, send, manage = false, disabled = false }: {
  member: CustomMember; me: boolean; state: AgentState;
  send: AgentClient['send']; manage?: boolean; disabled?: boolean;
}) {
  const summoner = state.summoner;
  const name = me && summoner?.gameName ? `${summoner.gameName}${summoner.tagLine ? `#${summoner.tagLine}` : ''}`
    : playerName(member.puuid, member.summonerId, state.friends);
  return (
    <div className={cx('flex min-h-14 items-center gap-3 rounded-tile px-3 py-2', me ? 'bg-accent-soft' : 'bg-raised')}>
      <Avatar name={name} src={assetUrl('profileIcon', member.summonerIconId)} size={40} />
      <span className="min-w-0 flex-1 truncate text-body font-semibold">{name}</span>
      {me && <span className="text-caption text-muted">Você</span>}
      {member.isLeader && (
        <span role="img" aria-label="Líder do grupo" title="Líder do grupo"
          className="flex size-6 shrink-0 items-center justify-center rounded-full bg-emerald-soft text-warn">
          <CrownIcon size={14} />
        </span>
      )}
      {/* Tornar líder e remover do grupo (#267): só o líder, nos outros jogadores. */}
      {manage && !me && <LobbyMemberMenu name={name} summonerId={member.summonerId} send={send} disabled={disabled} />}
    </div>
  );
}

function championName(state: AgentState, id: number): string {
  return state.championCatalog?.champions.find(c => c.id === id)?.name ?? `Campeão ${id}`;
}

function BotRow({ bot, state, editable, onChampion, onChange, onRemove }: {
  bot: CustomBot; state: AgentState; editable: boolean;
  onChampion: () => void;
  onChange: (change: Partial<Pick<CustomBot, 'difficulty' | 'position'>>) => void;
  onRemove: () => void;
}) {
  const name = championName(state, bot.championId);
  const difficultyId = useId();
  const positionId = useId();
  return (
    <div className="flex min-h-14 flex-wrap items-center gap-2 rounded-tile bg-raised px-3 py-2">
      {editable ? (
        <button type="button" aria-label={`Trocar o campeão do bot (${name})`} title="Trocar o campeão" aria-haspopup="dialog"
          onClick={onChampion} className={cx('flex min-w-0 flex-1 items-center gap-2 rounded-control text-left hover:bg-surface', focusRing)}>
          <ChampionIcon id={bot.championId} size={40} />
          <span className="min-w-0 truncate text-body font-semibold">{name}</span>
        </button>
      ) : (
        <span className="flex min-w-0 flex-1 items-center gap-2">
          <ChampionIcon id={bot.championId} size={40} />
          <span className="min-w-0 truncate text-body font-semibold">{name}</span>
        </span>
      )}
      <label htmlFor={positionId} className="sr-only">Posição do bot {name}</label>
      <span className="relative flex items-center">
        <PositionIcon position={bot.position} className="pointer-events-none absolute left-2 z-10" />
        <select id={positionId} value={bot.position} disabled={!editable} className={`${selectClass} pl-8`}
          onChange={event => onChange({ position: event.target.value as Position })}>
          {!(BOT_POSITIONS as readonly Position[]).includes(bot.position) && <option value={bot.position} disabled>Sem posição</option>}
          {BOT_POSITIONS.map(position => <option key={position} value={position}>{positionNames[position]}</option>)}
        </select>
      </span>
      <label htmlFor={difficultyId} className="sr-only">Dificuldade do bot {name}</label>
      <select id={difficultyId} value={bot.difficulty} disabled={!editable} className={selectClass}
        onChange={event => onChange({ difficulty: event.target.value as BotDifficulty })}>
        {bot.difficulty === 'unknown' && <option value="unknown" disabled>Dificuldade</option>}
        {DIFFICULTIES.map(d => <option key={d} value={d}>{botDifficultyNames[d]}</option>)}
      </select>
      {editable && (
        <button type="button" aria-label={`Tirar o bot ${name}`} title="Tirar o bot" onClick={onRemove}
          className={`flex size-8 items-center justify-center rounded-full text-muted transition-colors hover:text-danger ${focusRing}`}>
          <CloseIcon />
        </button>
      )}
    </div>
  );
}

/** Pop-up de escolha do campeão do bot: busca por nome e grade (#229). */
function ChampionPicker({ state, current, title, onChoose, onClose }: {
  state: AgentState; current: number | null; title: string; onChoose: (championId: number) => void; onClose: () => void;
}) {
  const [query, setQuery] = useState('');
  const titleId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const closeRef = useRef(onClose);
  useEffect(() => { closeRef.current = onClose; });
  useEffect(() => {
    inputRef.current?.focus();
    function onKey(event: KeyboardEvent) { if (event.key === 'Escape') closeRef.current(); }
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, []);
  const catalog = state.championCatalog?.champions ?? [];
  const allowed = state.availableBots ? new Set(state.availableBots) : null;
  const champions: Champion[] = catalog.filter(c => c.id > 0 && (!allowed || allowed.has(c.id)))
    .sort((a, b) => a.name.localeCompare(b.name, 'pt-BR'));
  const shown = searchChampions(champions, query);
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/40 p-4"
      onPointerDown={event => { if (event.target === event.currentTarget) onClose(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby={titleId}
        className="flex max-h-[80vh] w-[min(36rem,100%)] flex-col gap-3 rounded-panel bg-surface card p-4 text-ink shadow-lift animate-fade-up">
        <header className="flex items-center justify-between gap-2">
          <h3 id={titleId} className="text-section font-semibold">{title}</h3>
          <button type="button" aria-label="Fechar" onClick={onClose}
            className={`flex size-8 items-center justify-center rounded-full text-muted hover:text-ink ${focusRing}`}><CloseIcon /></button>
        </header>
        <input ref={inputRef} type="search" aria-label="Buscar campeão" placeholder="Buscar campeão" value={query}
          onChange={event => setQuery(event.target.value)}
          className={`h-10 rounded-control border border-control bg-raised px-3 text-body text-ink ${focusRing}`} />
        {!state.championCatalog ? <p className="px-1 text-body text-muted">Carregando campeões…</p>
          : shown.length === 0 ? <p className="px-1 text-body text-muted">Nenhum campeão encontrado.</p>
          : (
            <ul aria-label="Campeões" className="grid min-h-0 grid-cols-[repeat(auto-fill,minmax(4.5rem,1fr))] gap-1 overflow-y-auto pr-1">
              {shown.map(champion => (
                <li key={champion.id}>
                  <button type="button" aria-pressed={current === champion.id} onClick={() => onChoose(champion.id)} title={champion.name}
                    className={cx('flex w-full flex-col items-center gap-1 rounded-control p-1 text-caption text-ink hover:bg-raised aria-pressed:bg-accent-soft aria-pressed:text-accent-ink', focusRing)}>
                    <ChampionIcon id={champion.id} size={48} />
                    <span className="w-full truncate text-center">{champion.name}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
      </div>
    </div>
  );
}
