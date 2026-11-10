import type { AgentClient } from '../agent/client';
import { arenaTeams } from '../agent/arenaTeams';
import { assetUrl } from '../agent/assets';
import { memberName } from '../agent/friends';
import { awayMembers } from '../agent/lobbyRestrictions';
import { useCommand } from '../agent/useCommand';
import type { AgentState } from '../bindings/AgentState';
import type { Lobby } from '../bindings/Lobby';
import type { LobbyMember } from '../bindings/LobbyMember';
import type { SubteamSlot } from '../bindings/SubteamSlot';
import { Avatar } from './base';
import { cx } from './base/cx';
import { CrownIcon, PlusIcon, SwapIcon } from './home/icons';
import { LobbyMemberMenu } from './LobbyMemberMenu';

type Props = {
  lobby: Lobby;
  state: AgentState;
  localName: string;
  send: AgentClient['send'];
  /** Se dá para trocar de vaga agora (fase de lobby, fora da busca). */
  canSwap: boolean;
  /** Se o líder pode passar a liderança ou remover alguém agora (#267). */
  canManageMembers: boolean;
  /** Vaga com a lista de convite aberta, contada de 0 em todas as equipes. */
  picker: number | null;
  onPicker: (index: number | null) => void;
  /** Abre o perfil de outro jogador ao clicar na vaga dele (#308). */
  onOpenProfile?: (puuid: string) => void;
};

const focusRing = 'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent';
const iconButton = `flex size-9 shrink-0 items-center justify-center rounded-full bg-raised text-ink transition-colors enabled:hover:bg-accent enabled:hover:text-on-accent disabled:cursor-not-allowed disabled:text-muted ${focusRing}`;

/**
 * Lobby da Arena (#224): as vagas em equipes (6 de 3 na Arena 3x6), como no
 * client oficial. Cada vaga livre tem convidar e trocar de vaga, para o
 * jogador montar a própria equipe dentro do lobby.
 */
export function ArenaTeams({ lobby, state, localName, send, canSwap, canManageMembers, picker, onPicker, onOpenProfile }: Props) {
  const swap = useCommand(send);
  const layout = arenaTeams(lobby);
  if (!layout) return null;
  const me = state.summoner?.puuid;
  const name = (member: LobbyMember) => member.puuid === me ? localName : memberName(member, state.friends);
  const size = lobby.subteams?.size ?? 0;
  // Quem ainda está na tela de fim de jogo (#346) fica esmaecido, como no client oficial.
  const away = new Set(awayMembers(lobby, state.gameflowPhase).map(member => member.puuid));
  function moveTo(slot: SubteamSlot, label: string) {
    if (!canSwap || swap.pending) return;
    void swap.run({ type: 'setSubteamSlot', team: slot.team, position: slot.position },
      { pending: `Trocando para a ${label}…`, failure: 'trocar de vaga' });
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <ol aria-label="Equipes do lobby" className="grid min-h-0 flex-1 grid-cols-2 gap-3 @min-[52rem]:grid-cols-3">
        {layout.teams.map((team, t) => {
          const teamName = `Equipe ${t + 1}`;
          const mine = team.slots.some(s => s.member?.puuid === me);
          return (
            <li key={team.team} aria-label={teamName} style={{ animationDelay: `${t * 70}ms` }}
              className={cx('relative flex min-w-0 flex-col gap-2 rounded-panel card p-3 animate-fade-up has-[[aria-haspopup=menu][aria-expanded=true]]:z-20',
                mine ? 'bg-surface shadow-lift ring-2 ring-accent/30' : 'surface shadow-soft')}>
              <h3 className="flex items-center justify-between gap-2 px-1 text-caption font-semibold uppercase tracking-[0.06em] text-muted">
                {teamName}
                {mine && <span className="rounded-full bg-accent-soft px-2 py-0.5 normal-case tracking-normal text-accent-ink">Sua equipe</span>}
              </h3>
              <ul aria-label={`Vagas da ${teamName}`} className="flex flex-col gap-2">
                {team.slots.map(({ slot, member }, p) => {
                  const slotName = `vaga ${p + 1} da ${teamName}`;
                  if (member) {
                    const isMe = member.puuid === me;
                    const displayName = name(member);
                    const isAway = away.has(member.puuid);
                    return (
                      <li key={p} aria-label={`${displayName}, ${slotName}`}
                        className={cx('relative flex min-h-12 items-center gap-3 rounded-tile px-2 py-2', isMe ? 'bg-accent-soft' : 'bg-raised',
                          !isMe && onOpenProfile && 'transition-colors hover:bg-line/40')}>
                        {/* A vaga de outro jogador abre o perfil dele (#308); o menu do líder fica por cima. */}
                        {!isMe && onOpenProfile && (
                          <button type="button" aria-label={`Ver perfil de ${displayName}`} title={`Ver perfil de ${displayName}`}
                            onClick={() => onOpenProfile(member.puuid)}
                            className={`absolute inset-0 cursor-pointer rounded-tile ${focusRing}`} />
                        )}
                        <Avatar name={displayName} src={assetUrl('profileIcon', member.summonerIconId)} size={36}
                          className={cx('ring-2 ring-surface', isAway && 'opacity-40 grayscale')} />
                        <div className={cx('min-w-0 flex-1', isAway && 'opacity-60')}>
                          <p className="flex items-center gap-1.5">
                            <span className="truncate text-body font-semibold">{displayName}</span>
                            {member.isLeader && (
                              <span role="img" aria-label="Líder do grupo" title="Líder do grupo"
                                className="flex size-5 shrink-0 items-center justify-center rounded-full bg-emerald-soft text-warn">
                                <CrownIcon size={12} />
                              </span>
                            )}
                          </p>
                          <p className="truncate text-caption text-muted">
                            {isAway ? <span className="font-semibold text-warn">Ainda não voltou ao lobby</span>
                              : <>{isMe ? 'Você · ' : ''}Nível {member.summonerLevel}</>}
                          </p>
                        </div>
                        {/* Tornar líder e remover do grupo (#267): só o líder, nas vagas dos outros. */}
                        {lobby.isLeader && !isMe && (
                          <LobbyMemberMenu name={displayName} summonerId={member.summonerId} send={send} disabled={!canManageMembers} align="start" allowedInvite={member.allowedInviteOthers} />
                        )}
                      </li>
                    );
                  }
                  const index = t * size + p;
                  return (
                    <li key={p} aria-label={`Vaga livre, ${slotName}`}
                      className="flex min-h-12 items-center gap-2 rounded-tile border-2 border-dashed border-line px-2 py-2 text-muted">
                      <span className="flex-1 truncate text-caption font-semibold">Vaga livre</span>
                      {lobby.canInvite && (
                        <button type="button" aria-label={`Convidar amigo para a ${slotName}`} title="Convidar amigo"
                          aria-expanded={picker === index} aria-haspopup="dialog"
                          onClick={() => onPicker(picker === index ? null : index)} className={iconButton}>
                          <PlusIcon size={16} />
                        </button>
                      )}
                      <button type="button" aria-label={`Trocar para a ${slotName}`} title="Trocar para esta vaga"
                        disabled={!canSwap || swap.pending} aria-busy={swap.pending}
                        onClick={() => moveTo(slot, slotName)} className={iconButton}>
                        <SwapIcon size={16} />
                      </button>
                    </li>
                  );
                })}
              </ul>
            </li>
          );
        })}
      </ol>
      {layout.unplaced.length > 0 && (
        <p className="text-caption text-muted">Aguardando vaga: {layout.unplaced.map(name).join(', ')}.</p>
      )}
      {swap.feedback.message && (
        <p role={swap.feedback.status === 'error' ? 'alert' : 'status'}
          className={swap.feedback.status === 'error' ? 'text-body text-danger' : 'text-body text-muted'}>{swap.feedback.message}</p>
      )}
    </div>
  );
}
