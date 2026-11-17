//! Os comandos da UI, cada um levado ao [`Hub`] (ou ao launcher).
//!
//! Os comandos estão nos mesmos blocos por assunto do [`Command`]: um
//! comando novo entra no bloco do assunto dele, não no fim.

use crate::health::Feature;
use crate::launcher::{self, LaunchOptions};
use crate::lobby::{MemberAction, SubteamSlot};
use crate::swaps::SwapIntent;

use super::protocol::{Command, CommandResult, ErrorCode};
use super::Hub;

/// Executa `command` e devolve a resposta para a UI. O `getState` mexe na
/// assinatura da sessão e é tratado por ela.
pub(super) async fn run(hub: &Hub, launch: &LaunchOptions, command: Command) -> CommandResult {
    if feature(&command).is_some_and(|f| hub.feature_unavailable(f).is_some()) {
        return CommandResult::error(
            ErrorCode::FeatureUnavailable,
            "esta função está desligada porque o LoL mudou numa atualização; use o client oficial",
        );
    }
    if leaves_party(&command) && hub.party_locked() {
        return CommandResult::error(
            ErrorCode::InvalidCommand,
            "não dá para trocar de grupo na seleção de campeões ou em partida",
        );
    }
    match command {
        // Canal e LoL
        Command::Ping | Command::GetState => CommandResult::Ok,
        Command::LaunchLeague => launch_league(launch).await,
        Command::LaunchUx => hub.launch_ux().await,

        // Lobby, fila e partida encontrada
        Command::CreateLobby { queue } => hub.create_lobby(queue).await,
        Command::CreateLobbyById { queue_id } => hub.create_lobby_by_id(queue_id).await,
        Command::LeaveLobby => hub.leave_lobby().await,
        Command::PlayAgain => hub.play_again().await,
        Command::LeavePostGame => hub.leave_post_game().await,
        Command::SetPositions { first, second } => hub.set_positions(first, second).await,
        Command::SetPositionOrder { positions } => hub.set_position_order(&positions).await,
        Command::StartQueue => hub.start_queue().await,
        Command::StopQueue => hub.stop_queue().await,
        Command::AcceptReadyCheck => hub.accept_ready_check().await,
        Command::DeclineReadyCheck => hub.decline_ready_check().await,
        Command::HonorPlayer { puuid } => hub.honor_player(&puuid).await,
        Command::SkipHonor => hub.skip_honor().await,
        Command::StartTutorial { queue_id } => hub.start_tutorial(queue_id).await,
        Command::StartPracticeTool => hub.start_practice_tool().await,
        Command::CreateCustomGame { options } => hub.create_custom_game(options).await,
        Command::SetPlayerSlots { slots } => hub.set_player_slots(slots).await,
        Command::SetLobbyOpen { open } => hub.set_lobby_open(open).await,
        Command::KickLobbyMember { summoner_id } => {
            hub.lobby_member_action(summoner_id, MemberAction::Kick)
                .await
        }
        Command::PromoteLobbyMember { summoner_id } => {
            hub.lobby_member_action(summoner_id, MemberAction::Promote)
                .await
        }
        Command::SetLobbyMemberInvite {
            summoner_id,
            allowed,
        } => {
            let action = if allowed {
                MemberAction::GrantInvite
            } else {
                MemberAction::RevokeInvite
            };
            hub.lobby_member_action(summoner_id, action).await
        }
        Command::CreateLobbyLink => hub.create_lobby_link().await,
        Command::SetSubteamSlot { team, position } => {
            hub.set_subteam_slot(SubteamSlot { team, position }).await
        }
        Command::AddBot {
            team,
            champion_id,
            difficulty,
            position,
        } => hub.add_bot(team, champion_id, difficulty, position).await,
        Command::UpdateBot {
            bot_uuid,
            champion_id,
            difficulty,
            position,
        } => {
            hub.update_bot(&bot_uuid, champion_id, difficulty, position)
                .await
        }
        Command::RemoveBot { bot_uuid } => hub.remove_bot(&bot_uuid).await,
        Command::SwitchTeam { team } => hub.switch_team(team).await,
        Command::Spectate => hub.spectate().await,
        Command::StartCustomGame => hub.start_custom_game().await,
        Command::RefreshCustomGames => hub.refresh_custom_games().await,
        Command::JoinCustomGame { party_id, password } => {
            hub.join_custom_game(&party_id, password.as_deref().unwrap_or_default())
                .await
        }

        // Seleção de campeões
        Command::HoverChampion {
            action_id,
            champion_id,
        } => hub.hover_champion(action_id, champion_id).await,
        Command::ConfirmAction {
            action_id,
            champion_id,
        } => hub.confirm_action(action_id, champion_id).await,
        Command::SetSummonerSpells {
            spell1_id,
            spell2_id,
        } => hub.set_summoner_spells(spell1_id, spell2_id).await,
        Command::SelectSkin { skin_id } => hub.select_skin(skin_id).await,
        Command::RequestSwap { kind, swap_id } => {
            hub.champ_select_swap(kind, swap_id, SwapIntent::Request)
                .await
        }
        Command::AcceptSwap { kind, swap_id } => {
            hub.champ_select_swap(kind, swap_id, SwapIntent::Accept)
                .await
        }
        Command::DeclineSwap { kind, swap_id } => {
            hub.champ_select_swap(kind, swap_id, SwapIntent::Decline)
                .await
        }
        Command::CancelSwap { kind, swap_id } => {
            hub.champ_select_swap(kind, swap_id, SwapIntent::Cancel)
                .await
        }
        Command::RerollChampion => hub.reroll_champion().await,
        Command::SwapBenchChampion { champion_id } => hub.swap_bench_champion(champion_id).await,
        Command::AckAutofill => hub.ack_autofill().await,
        Command::QuitCustomChampSelect => hub.quit_custom_champ_select().await,
        Command::SendChampSelectMessage { body } => hub.send_champ_select_message(&body).await,
        Command::SendLobbyMessage { body } => hub.send_lobby_message(&body).await,

        // Runas
        Command::SelectRunePage { page_id } => hub.select_rune_page(page_id).await,
        Command::CreateRunePage { page } => hub.create_rune_page(page).await,
        Command::UpdateRunePage { page_id, page } => hub.update_rune_page(page_id, page).await,
        Command::DeleteRunePage { page_id } => hub.delete_rune_page(page_id).await,

        // Social: amigos, convites e conversas
        Command::AcceptInvitation { invitation_id } => hub.accept_invitation(&invitation_id).await,
        Command::DeclineInvitation { invitation_id } => {
            hub.decline_invitation(&invitation_id).await
        }
        Command::InviteToLobby { invitees } => hub.invite_to_lobby(&invitees).await,
        Command::InviteByRiotId {
            game_name,
            tag_line,
        } => hub.invite_by_riot_id(&game_name, &tag_line).await,
        Command::LoadRecentPlayers => hub.load_recent_players().await,
        Command::LoadSuggestedPlayers => hub.load_suggested_players().await,
        Command::DismissSuggestedPlayer { summoner_id } => {
            hub.dismiss_suggested_player(summoner_id).await
        }
        Command::SendChatMessage {
            conversation_id,
            body,
        } => hub.send_chat_message(&conversation_id, &body).await,
        Command::SetActiveConversation { conversation_id } => {
            hub.set_active_conversation(conversation_id.as_deref())
                .await
        }

        // Perfil
        Command::LoadMoreMatches => hub.load_more_matches().await,
        Command::OpenMatchDetails { game_id } => hub.open_match_details(game_id).await,
        Command::OpenPlayerProfile { puuid } => hub.open_player_profile(&puuid).await,
        Command::LoadMorePlayerMatches => hub.load_more_player_matches().await,

        // Coleção
        Command::OpenCollection => hub.open_collection().await,
        Command::OpenChampionSkins { champion_id } => hub.open_champion_skins(champion_id).await,

        // Início
        Command::OpenPatchNotes => hub.open_patch_notes(),
    }
}

/// Comandos que tiram o jogador do grupo atual (#506): criar ou sair do
/// lobby, entrar em outro e aceitar convite. Na seleção de campeões isso é
/// esquiva (dodge, com penalidade), e na partida não faz sentido; nessas
/// fases ([`Hub::party_locked`]) o agente recusa sem falar com a LCU.
fn leaves_party(command: &Command) -> bool {
    matches!(
        command,
        Command::CreateLobby { .. }
            | Command::CreateLobbyById { .. }
            | Command::LeaveLobby
            | Command::StartTutorial { .. }
            | Command::StartPracticeTool
            | Command::CreateCustomGame { .. }
            | Command::JoinCustomGame { .. }
            | Command::AcceptInvitation { .. }
    )
}

/// A função do modo de compatibilidade (#36) de que o comando depende, ou
/// `None` para os que não usam os endpoints checados. Sem `_`: um comando
/// novo precisa dizer a função dele.
fn feature(command: &Command) -> Option<Feature> {
    use Command::*;
    match command {
        Ping
        | GetState
        | LaunchLeague
        | LaunchUx
        | OpenCollection
        | OpenChampionSkins { .. }
        | OpenPatchNotes => None,
        CreateLobby { .. }
        | CreateLobbyById { .. }
        | LeaveLobby
        | PlayAgain
        | SetPositions { .. }
        | SetPositionOrder { .. }
        | StartQueue
        | StopQueue
        | StartTutorial { .. }
        | StartPracticeTool
        | CreateCustomGame { .. }
        | SetPlayerSlots { .. }
        | SetLobbyOpen { .. }
        | KickLobbyMember { .. }
        | PromoteLobbyMember { .. }
        | SetLobbyMemberInvite { .. }
        | CreateLobbyLink
        | SetSubteamSlot { .. }
        | AddBot { .. }
        | UpdateBot { .. }
        | RemoveBot { .. }
        | SwitchTeam { .. }
        | Spectate
        | StartCustomGame
        | RefreshCustomGames
        | JoinCustomGame { .. }
        | SendLobbyMessage { .. }
        | AcceptInvitation { .. }
        | DeclineInvitation { .. }
        | InviteToLobby { .. }
        | InviteByRiotId { .. } => Some(Feature::Lobby),
        AcceptReadyCheck | DeclineReadyCheck => Some(Feature::ReadyCheck),
        HoverChampion { .. }
        | ConfirmAction { .. }
        | SetSummonerSpells { .. }
        | SelectSkin { .. }
        | RequestSwap { .. }
        | AcceptSwap { .. }
        | DeclineSwap { .. }
        | CancelSwap { .. }
        | RerollChampion
        | SwapBenchChampion { .. }
        | AckAutofill
        | QuitCustomChampSelect
        | SendChampSelectMessage { .. } => Some(Feature::ChampSelect),
        SelectRunePage { .. }
        | CreateRunePage { .. }
        | UpdateRunePage { .. }
        | DeleteRunePage { .. } => Some(Feature::Runes),
        LoadRecentPlayers
        | LoadSuggestedPlayers
        | DismissSuggestedPlayer { .. }
        | SendChatMessage { .. }
        | SetActiveConversation { .. } => Some(Feature::Social),
        LoadMoreMatches
        | OpenMatchDetails { .. }
        | OpenPlayerProfile { .. }
        | LoadMorePlayerMatches => Some(Feature::Profile),
        HonorPlayer { .. } | SkipHonor | LeavePostGame => Some(Feature::PostGame),
    }
}

/// Abre o LoL e traduz o resultado para a UI. Não espera o LoL subir: isso
/// chega pelas mensagens `lcu`.
async fn launch_league(options: &LaunchOptions) -> CommandResult {
    match launcher::launch_league(options).await {
        Ok(_) => CommandResult::Ok,
        Err(e) => CommandResult::error(ErrorCode::LaunchFailed, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameflow::GameflowPhase;
    use crate::lobby::Queue;

    fn is_invalid(result: &CommandResult) -> bool {
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        )
    }

    #[tokio::test]
    async fn recusa_trocar_de_grupo_na_selecao_e_na_partida() {
        let launch = LaunchOptions::default();
        for phase in [
            GameflowPhase::ChampSelect,
            GameflowPhase::GameStart,
            GameflowPhase::InProgress,
            GameflowPhase::Reconnect,
        ] {
            let hub = Hub::new();
            hub.set_gameflow_phase(Some(phase));
            for command in [
                Command::AcceptInvitation {
                    invitation_id: "convite-1".into(),
                },
                Command::LeaveLobby,
                Command::CreateLobby { queue: Queue::Aram },
                Command::StartPracticeTool,
            ] {
                let result = run(&hub, &launch, command.clone()).await;
                assert!(is_invalid(&result), "{phase:?} {command:?}: {result:?}");
            }
        }
    }

    #[tokio::test]
    async fn fora_da_selecao_o_comando_segue_para_a_lcu() {
        // Sem LoL conectado, o comando chega ao hub, que responde
        // `notConnected` em vez de `invalidCommand`.
        let hub = Hub::new();
        hub.set_gameflow_phase(Some(GameflowPhase::Lobby));
        let result = run(&hub, &LaunchOptions::default(), Command::LeaveLobby).await;
        assert!(!is_invalid(&result), "{result:?}");
    }
}
