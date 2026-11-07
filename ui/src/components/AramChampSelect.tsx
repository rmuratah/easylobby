import { championName } from '../agent/champSelect';
import { useCommand, type CommandFeedback } from '../agent/useCommand';
import type { ChampSelect } from '../bindings/ChampSelect';
import type { ChampionCatalog } from '../bindings/ChampionCatalog';
import { Button } from './base';
import { ChampSelectChatPanel } from './ChampSelectChatPanel';
import { ChampionIcon, Countdown, TeamList, type ContentProps } from './ChampSelectPanel';
import { ChampSelectSwaps, SwapOffers } from './ChampSelectSwaps';
import { RuneQuickPick } from './RuneQuickPick';
import { SummonerSpellsPicker } from './SummonerSpellsPicker';
import { champSelectGameMode } from '../agent/summonerSpells';

/**
 * Tela da seleção do ARAM e da ARAM: Desordem (#201), com as cartas de
 * campeão (patch 25.13). Na abertura, a LCU sorteia 2 ou 3 cartas para cada
 * jogador (`champSelectChampions.subset`) e deixa um pick em andamento:
 * escolher a carta é confirmar esse pick. As cartas que sobram vão para o
 * banco do time, de onde qualquer um pega um campeão (o dele vai para o
 * banco no lugar). As trocas com aliados (#25), os feitiços e o chat
 * continuam como no draft, e as runas também, menos na ARAM: Desordem. O reroll antigo só aparece
 * se a LCU mandar reroll sobrando.
 */
export function AramChampSelectContent({ state, champSelect, send, onOpenRunes }: ContentProps) {
  const catalog = state.championCatalog;
  const me = champSelect.myTeam.find(p => p.isLocalPlayer) ?? null;
  const reroll = useCommand(send);
  const pick = useCommand(send);
  const bench = useCommand(send);
  const swap = useCommand(send);
  const starting = champSelect.timer.phase === 'GAME_STARTING';
  // Nome da fila no idioma do client (ARAM, ARAM: Desordem), como na lista de filas.
  const mode = state.queues.find(q => q.queueId === champSelect.queueId)?.name || 'ARAM';
  // No ARAM dá para trocar as runas; na ARAM: Desordem (`KIWI`), não (#197).
  const runes = champSelectGameMode(champSelect, state.queues) !== 'KIWI';
  const busy = reroll.pending || bench.pending || pick.pending || starting;
  // A escolha da carta é o pick em andamento do jogador.
  const choice = champSelect.pendingAction?.type === 'pick' ? champSelect.pendingAction : null;
  function chooseCard(championId: number) {
    if (busy || !choice) return;
    void pick.run({ type: 'confirmAction', actionId: choice.id, championId },
      { pending: `Escolhendo ${championName(catalog, championId)}…`, failure: 'escolher a carta' });
  }
  function rollAgain() {
    if (busy) return;
    void reroll.run({ type: 'rerollChampion' }, { pending: 'Rolando de novo…', failure: 'rolar de novo' });
  }
  function takeFromBench(championId: number) {
    if (busy) return;
    void bench.run({ type: 'swapBenchChampion', championId },
      { pending: `Pegando ${championName(catalog, championId)} do banco…`, failure: 'pegar o campeão do banco' });
  }
  return (
    <section aria-label="Seleção de campeões" className="champ-select text-ink">
      <div className="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-card border border-line bg-surface px-5 py-3 text-ink">
        <div className="flex min-w-0 flex-1 flex-col">
          <p className="text-caption text-muted">Modo: <strong className="font-semibold text-ink">{mode}</strong></p>
          <p role="status" className="text-section font-semibold text-accent-ink">
            {starting ? 'A partida está abrindo.' : choice ? 'Escolha uma das suas cartas.' : 'Troque com o banco ou com um aliado.'}
          </p>
        </div>
        <p className="flex items-baseline gap-2 text-caption text-muted">Tempo:
          <span className="text-[2rem] font-semibold leading-none tabular-nums text-ink">
            <Countdown key={`${champSelect.timer.phase}:${champSelect.timer.endsAtMs}`} champSelect={champSelect} />
          </span>
        </p>
      </div>
      <div className="champ-select-board">
        <div className="flex min-w-0 flex-col gap-4">
          <TeamList label="Seu time" players={champSelect.myTeam} ally acting={new Set()} catalog={catalog}
            renderActions={player => player.isLocalPlayer ? null
              : <SwapOffers champSelect={champSelect} cellId={player.cellId} busy={swap.pending} run={swap.run} />} />
          <SummonerSpellsPicker champSelect={champSelect} catalog={state.summonerSpellCatalog}
            summonerLevel={state.summoner?.summonerLevel ?? null} queues={state.queues} send={send} />
          {runes && <RuneQuickPick runePages={state.runePages} catalog={state.runeCatalog} send={send} onEdit={onOpenRunes} />}
        </div>
        <div className="champ-select-center">
          {choice ? (
            <ChampionCards offers={state.champSelectChampions.subset} catalog={catalog} busy={busy}
              onChoose={chooseCard} feedback={pick.feedback} />
          ) : (
            <MyChampion championId={me?.championId ?? null} champSelect={champSelect} catalog={catalog}
              busy={busy} onReroll={rollAgain} feedback={[pick.feedback, reroll.feedback, bench.feedback]} />
          )}
          <ChampSelectSwaps champSelect={champSelect} busy={swap.pending} run={swap.run} feedback={swap.feedback} />
          <Bench champSelect={champSelect} catalog={catalog} busy={busy} onTake={takeFromBench} />
        </div>
        <div className="flex min-w-0 flex-col gap-4">
          {state.gameflowPhase === 'ChampSelect' && <ChampSelectChatPanel champSelect={champSelect} chat={state.champSelectChat} send={send} />}
        </div>
      </div>
    </section>
  );
}

type CardsProps = {
  offers: number[] | null;
  catalog: ChampionCatalog | null;
  busy: boolean;
  onChoose: (championId: number) => void;
  feedback: CommandFeedback;
};

/** As cartas de campeão do jogador: clicar numa confirma a escolha; as outras vão para o banco. */
function ChampionCards({ offers, catalog, busy, onChoose, feedback }: CardsProps) {
  return (
    <section aria-label="Suas cartas" className="flex flex-col gap-3 rounded-card border-2 border-accent bg-surface p-4 text-ink">
      <div className="flex flex-wrap items-baseline gap-x-3">
        <h3 className="text-section font-semibold">Suas cartas</h3>
        <p className="text-caption text-muted">Fique com uma. As outras vão para o banco do time.</p>
      </div>
      {!offers || offers.length === 0 ? (
        <p role="status" className="text-body text-muted">Carregando suas cartas…</p>
      ) : (
        <ul className="grid grid-cols-3 gap-2" aria-label="Cartas de campeão">
          {offers.map(championId => {
            const name = championName(catalog, championId);
            return (
              <li key={championId}>
                <button type="button" disabled={busy} aria-label={`Escolher ${name}`} onClick={() => onChoose(championId)}
                  className="flex w-full min-w-0 flex-col items-center gap-2 rounded-card border-2 border-line bg-raised p-3 text-body font-semibold text-ink hover:border-accent hover:bg-accent-soft focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-50">
                  <ChampionIcon id={championId} size={80} />
                  <span className="w-full truncate text-center">{name}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {feedback.status !== 'idle' && <p role={feedback.status === 'error' ? 'alert' : 'status'}
        className={feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{feedback.message}</p>}
    </section>
  );
}

type MyChampionProps = {
  championId: number | null;
  champSelect: ChampSelect;
  catalog: ChampionCatalog | null;
  busy: boolean;
  onReroll: () => void;
  feedback: CommandFeedback[];
};

function MyChampion({ championId, champSelect, catalog, busy, onReroll, feedback }: MyChampionProps) {
  const shown = feedback.find(f => f.status !== 'idle');
  const rerolls = champSelect.rerollsRemaining;
  const canReroll = champSelect.allowRerolling && rerolls > 0;
  return (
    <section aria-label="Seu campeão" className="flex flex-wrap items-center gap-4 rounded-card border-2 border-accent bg-surface p-4 text-ink">
      <ChampionIcon id={championId} size={96} />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        <p className="text-caption text-muted">Seu campeão</p>
        <p className="text-[1.5rem] font-semibold leading-tight">{championId ? championName(catalog, championId) : 'Sorteando…'}</p>
        {/* Desde o 25.13 o ARAM usa cartas e a LCU manda `allowRerolling` com 0 rerolls: o botão só aparece com reroll sobrando. */}
        {canReroll && (
          <div className="flex flex-wrap items-center gap-3">
            <Button variant="primary" disabled={busy} onClick={onReroll}>Rolar de novo</Button>
            <p className="text-body text-muted" aria-label="Rerolls restantes">
              {rerolls === 1 ? '1 reroll restante' : `${rerolls} rerolls restantes`}
            </p>
          </div>
        )}
        {shown && <p role={shown.status === 'error' ? 'alert' : 'status'}
          className={shown.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{shown.message}</p>}
      </div>
    </section>
  );
}

type BenchProps = { champSelect: ChampSelect; catalog: ChampionCatalog | null; busy: boolean; onTake: (championId: number) => void };

function Bench({ champSelect, catalog, busy, onTake }: BenchProps) {
  const champions = champSelect.benchChampions;
  return (
    <section aria-label="Banco" className="flex min-w-0 flex-col gap-3 rounded-card border border-line bg-surface p-4 text-ink">
      <div className="flex flex-wrap items-baseline gap-x-3">
        <h3 className="text-section font-semibold">Banco</h3>
        <p className="text-caption text-muted">Clique num campeão para trocar pelo seu. O seu vai para o banco.</p>
      </div>
      {champions.length === 0 ? (
        <p className="text-body text-muted">Nenhum campeão no banco ainda. As cartas que o time não escolher aparecem aqui.</p>
      ) : (
        <ul className="champion-grid" aria-label="Campeões no banco">
          {champions.map(({ championId, isPriority }) => {
            const name = championName(catalog, championId);
            return (
              <li key={championId}>
                <button type="button" disabled={busy} title={`Pegar ${name}`} aria-label={`Pegar ${name} do banco`}
                  onClick={() => onTake(championId)}
                  className="flex w-full flex-col items-center gap-1 rounded-control border-2 border-transparent p-1 text-caption text-ink hover:border-accent hover:bg-accent-soft focus-visible:outline-2 focus-visible:outline-accent disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-transparent disabled:hover:bg-transparent">
                  <ChampionIcon id={championId} size={56} />
                  <span className="w-full truncate text-center">{name}</span>
                  {isPriority && <span className="text-caption font-semibold text-accent-ink">prioridade</span>}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
