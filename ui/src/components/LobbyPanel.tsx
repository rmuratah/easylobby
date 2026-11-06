import { useEffect, useId, useRef, useState } from 'react';
import type { AgentClient, ConnectionSnapshot } from '../agent/client';
import { isQueue, queueNames } from '../agent/queues';
import type { AgentState } from '../bindings/AgentState';
import type { Command } from '../bindings/Command';
import { isPosition, positionNames, validPositions } from '../agent/positions';
import type { Position } from '../bindings/Position';
import type { Queue } from '../bindings/Queue';
import { assetUrl } from '../agent/assets';
import { Avatar, Button } from './base';
import { cx } from './base/cx';

type Props = { connection: ConnectionSnapshot; send: AgentClient['send'] };
type LobbyCommand = Extract<Command, { type: 'createLobby' | 'leaveLobby' | 'setPositions' }>;
type Feedback = { status: 'idle' | 'pending' | 'success' | 'error'; message: string };
const selectClass = 'h-10 rounded-control border border-control bg-surface px-3 text-body text-ink disabled:cursor-not-allowed disabled:text-muted focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent';

export function LobbyPanel({ connection, send }: Props) {
  const state = connection.state;
  const lcu = state?.lcu;
  if (connection.status !== 'connected' || !state || lcu?.state !== 'connected') {
    return (
      <section aria-label="Lobby" className="flex w-full flex-col gap-2 rounded-card border border-line bg-surface p-5 text-ink">
        <h2 className="text-section font-semibold">Lobby</h2>
        <p className="text-body text-muted">Aguarde a conexão com o agente e o League of Legends. Confira o lobby após reconectar antes de tentar novamente.</p>
      </section>
    );
  }
  // Trocar a sessão ou o jogador descarta feedback e respostas da sessão anterior.
  return <LobbyActions key={`${lcu.session}:${state.summoner?.puuid ?? ''}`} state={state} send={send} />;
}

function LobbyActions({ state, send }: { state: AgentState; send: AgentClient['send'] }) {
  const { lobby, matchmaking, summoner, gameflowPhase: phase } = state;
  const localMember = summoner?.puuid ? lobby?.members.find(member => member.puuid === summoner.puuid) : undefined;
  const first = localMember?.firstPosition ?? 'unselected';
  const second = localMember?.secondPosition ?? 'unselected';
  const confirmedKey = `${lobby?.queueId}:${lobby?.showPositionSelector}:${localMember?.puuid}:${first}:${second}`;
  const [positions, setPositions] = useState({ confirmedKey, first, second });
  const [acceptedPositions, setAcceptedPositions] = useState<typeof positions | null>(null);
  if (positions.confirmedKey !== confirmedKey) {
    setPositions({ confirmedKey, first, second });
    setAcceptedPositions(null);
  }
  const queueId = lobby?.queueId ?? null;
  const [selection, setSelection] = useState<{ queueId: number | null; queue: Queue }>({ queueId, queue: lobby?.queue ?? 'normalDraft' });
  if (selection.queueId !== queueId) {
    setSelection({ queueId, queue: lobby?.queue ?? 'normalDraft' });
  }
  const [feedback, setFeedback] = useState<Feedback>({ status: 'idle', message: '' });
  const inFlight = useRef(false);
  const mounted = useRef(false);
  const descriptionId = useId();
  const selectId = useId();
  const firstId = useId();
  const secondId = useId();
  const positionContext = useRef(0);
  const confirmedRevision = useRef(0);
  useEffect(() => { confirmedRevision.current += 1; }, [confirmedKey]);
  useEffect(() => {
    positionContext.current += 1;
  }, [queueId, lobby?.showPositionSelector, localMember?.puuid]);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const phaseAllowsChange = !state.readyCheck && !matchmaking && (phase === 'None' || phase === 'Lobby');
  const canCreate = !!summoner && phaseAllowsChange && (!lobby || lobby.isLeader);
  const canLeave = !state.readyCheck && !!summoner && !!lobby && !matchmaking && phase === 'Lobby';
  const canSetPositions = !state.readyCheck && !!localMember && !!lobby?.showPositionSelector && !matchmaking && phase === 'Lobby';
  const positionsChanged = (positions.first !== first || positions.second !== second) &&
    !(acceptedPositions?.confirmedKey === confirmedKey && acceptedPositions.first === positions.first && acceptedPositions.second === positions.second);
  const pending = feedback.status === 'pending';
  const localName = summoner?.gameName
    ? `${summoner.gameName}${summoner.tagLine ? `#${summoner.tagLine}` : ''}`
    : summoner?.displayName || 'Você';
  const restriction = !summoner ? 'Aguarde o login no League of Legends.'
    : !phaseAllowsChange ? 'Criar, trocar fila e sair do lobby só estão disponíveis fora da busca e da partida. Aguarde a fase de lobby.'
    : lobby && !lobby.isLeader ? 'Somente o líder pode trocar a fila. Você pode sair do lobby.'
    : 'Escolha uma fila para criar um lobby ou trocar a fila atual.';

  async function execute(command: LobbyCommand) {
    if (inFlight.current || (command.type === 'createLobby'
      ? !canCreate || command.queue === lobby?.queue
      : command.type === 'leaveLobby' ? !canLeave
      : !canSetPositions || !positionsChanged || !validPositions(command.first, command.second))) return;
    const context = positionContext.current;
    const revision = confirmedRevision.current;
    inFlight.current = true;
    setFeedback({ status: 'pending', message: command.type === 'setPositions' ? 'Enviando cargos…' : command.type === 'leaveLobby' ? 'Saindo do lobby…'
      : lobby ? 'Trocando fila…' : 'Criando lobby…' });
    try {
      const result = await send(command);
      if (!mounted.current) return;
      if (command.type === 'setPositions' && context !== positionContext.current) {
        setFeedback({ status: 'idle', message: '' });
        return;
      }
      if (result.status === 'ok') {
        // Evita repetir um comando aceito enquanto o evento ainda não chegou.
        if (command.type === 'setPositions' && revision === confirmedRevision.current) {
          setAcceptedPositions({ confirmedKey, first: command.first, second: command.second });
        }
        setFeedback({ status: 'success', message: command.type === 'setPositions'
          ? 'Comando de cargos aceito. Confira as posições confirmadas pelo agente.' : command.type === 'leaveLobby'
          ? 'Saída do lobby confirmada.' : 'Lobby atualizado pelo agente.' });
      } else {
        setFeedback({ status: 'error', message: result.code === 'notConnected'
          ? 'O League of Legends desconectou. Aguarde a reconexão e confira o lobby antes de tentar novamente.'
          : `Não foi possível alterar o lobby: ${result.message}` });
      }
    } catch {
      if (!mounted.current) return;
      setFeedback({ status: 'error', message: 'Não foi possível confirmar o comando. Confira a conexão e o lobby antes de tentar novamente.' });
    } finally {
      inFlight.current = false;
    }
  }

  const searching = !!matchmaking || !!state.readyCheck;
  const queueLabel = lobby ? (lobby.queue ? queueNames[lobby.queue] : `Outra fila (ID ${lobby.queueId})`) : null;
  return (
    <section aria-label="Lobby" className="flex w-full flex-col gap-5 rounded-card border border-line bg-surface p-5 text-ink">
      <header className="flex flex-col gap-1">
        <h2 className="text-caption font-semibold uppercase tracking-[0.06em] text-muted">Lobby</h2>
        {lobby ? (
          <>
            <p className="text-title font-semibold tracking-[-0.015em]"><span className="sr-only">Fila atual: </span><strong className="font-semibold">{queueLabel}</strong></p>
            <p className="text-body text-muted">{lobby.members.length} / {lobby.maxLobbySize} jogadores · {lobby.isLeader ? 'Você é o líder' : 'Você é membro'}</p>
          </>
        ) : <p className="text-title font-semibold tracking-[-0.015em]">Você não está em um lobby.</p>}
      </header>
      {lobby && (
        <ul aria-label="Jogadores no lobby" className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-2.5">
          {lobby.members.map(member => {
            const me = member.puuid === summoner?.puuid;
            const name = me ? localName : `Invocador ${member.summonerId}`;
            return (
              <li key={member.puuid} className={cx('flex flex-col items-center gap-2 rounded-card border p-3 text-center',
                me ? 'border-accent bg-accent-soft' : 'border-line bg-surface')}>
                <Avatar name={me && summoner?.gameName ? summoner.gameName : name} src={assetUrl('profileIcon', member.summonerIconId)} size={48} />
                <div className="break-words text-body font-semibold">{name}{' · Nível '}{member.summonerLevel}{member.isLeader ? ' · Líder' : ''}</div>
                {lobby.showPositionSelector && (
                  <span className="text-caption text-muted">
                    Primário: {positionNames[member.firstPosition]} · Secundário: {positionNames[member.secondPosition]}
                  </span>
                )}
              </li>
            );
          })}
          {Array.from({ length: Math.max(0, lobby.maxLobbySize - lobby.members.length) }, (_, index) => (
            <li key={`vaga-${index}`} aria-label="Vaga livre"
              className="flex min-h-[7rem] items-center justify-center rounded-card border border-dashed border-line text-caption text-muted">
              Vaga livre
            </li>
          ))}
        </ul>
      )}
      {lobby && (lobby.showPositionSelector ? (
        <fieldset className="flex flex-col gap-3" disabled={!canSetPositions || pending}>
          <legend className="mb-1 text-section font-semibold">Seus cargos</legend>
          <p className="text-caption text-muted">As posições na lista de jogadores são confirmadas pelo agente.</p>
          {!localMember && <p className="text-body text-muted">Aguarde a identificação do seu membro no lobby.</p>}
          {phase !== 'Lobby' && <p className="text-body text-muted">A escolha de cargos está disponível na fase de lobby.</p>}
          <div className="flex flex-wrap items-end gap-3">
            <label className="flex min-w-[10rem] flex-1 flex-col gap-1.5 text-caption font-medium text-muted" htmlFor={firstId}>Cargo primário
              <select id={firstId} value={positions.first} className={selectClass}
                onChange={event => {
                  if (!isPosition(event.target.value)) return;
                  const next = event.target.value;
                  setPositions({ ...positions, first: next,
                    second: validPositions(next, positions.second) ? positions.second : 'unselected' });
                }}>
                {(Object.keys(positionNames) as Position[]).map(position =>
                  <option key={position} value={position}>{positionNames[position]}</option>)}
              </select>
            </label>
            <label className="flex min-w-[10rem] flex-1 flex-col gap-1.5 text-caption font-medium text-muted" htmlFor={secondId}>Cargo secundário
              <select id={secondId} value={positions.second} className={selectClass}
                disabled={!canSetPositions || pending || positions.first === 'fill' || positions.first === 'unselected'}
                onChange={event => {
                  if (isPosition(event.target.value)) setPositions({ ...positions, second: event.target.value });
                }}>
                {(Object.keys(positionNames) as Position[]).map(position =>
                  <option key={position} value={position} disabled={!validPositions(positions.first, position)}>{positionNames[position]}</option>)}
              </select>
            </label>
            <Button aria-busy={pending}
              disabled={!canSetPositions || pending || !positionsChanged || !validPositions(positions.first, positions.second)}
              onClick={() => { void execute({ type: 'setPositions', first: positions.first, second: positions.second }); }}>
              Salvar cargos
            </Button>
          </div>
        </fieldset>
      ) : <p className="text-body text-muted">Esta fila não permite escolher cargos.</p>)}
      {/* Na fila e na partida encontrada, a faixa de ação fica com a busca; trocar fila e sair voltam depois. */}
      {!searching && (
        <div className="flex flex-col gap-3 border-t border-line pt-4">
          <p id={descriptionId} className="text-caption text-muted">{restriction}</p>
          <div className="flex flex-wrap items-end gap-3">
            <label className="flex min-w-[14rem] flex-1 flex-col gap-1.5 text-caption font-medium text-muted" htmlFor={selectId}>Fila
              <select
                id={selectId}
                value={selection.queue}
                disabled={!canCreate || pending}
                aria-describedby={descriptionId}
                onChange={event => {
                  if (isQueue(event.target.value)) setSelection({ queueId, queue: event.target.value });
                }}
                className={selectClass}
              >
                {(Object.keys(queueNames) as Queue[]).map(queue => <option key={queue} value={queue}>{queueNames[queue]}</option>)}
              </select>
            </label>
            <Button
              variant={lobby ? 'secondary' : 'primary'} aria-describedby={descriptionId} aria-busy={pending}
              disabled={!canCreate || pending || selection.queue === lobby?.queue}
              onClick={() => { void execute({ type: 'createLobby', queue: selection.queue }); }}
            >{lobby ? 'Trocar fila' : 'Criar lobby'}</Button>
            {lobby && <Button
              variant="ghost" aria-describedby={descriptionId} aria-busy={pending}
              disabled={!canLeave || pending}
              onClick={() => { void execute({ type: 'leaveLobby' }); }}
            >Sair do lobby</Button>}
          </div>
        </div>
      )}
      {feedback.message && <p role={feedback.status === 'error' ? 'alert' : 'status'}
        className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>}
    </section>
  );
}
