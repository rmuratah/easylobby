//! Os comandos da UI, cada um levado ao [`Hub`] (ou ao launcher).
//!
//! Os comandos estão nos mesmos blocos por assunto do [`Command`]: um
//! comando novo entra no bloco do assunto dele, não no fim.

use crate::launcher::{self, LaunchOptions};
use crate::lobby::SubteamSlot;
use crate::swaps::SwapIntent;

use super::protocol::{Command, CommandResult, ErrorCode};
use super::Hub;

/// Executa `command` e devolve a resposta para a UI. O `getState` mexe na
/// assinatura da sessão e é tratado por ela.
pub(super) async fn run(hub: &Hub, launch: &LaunchOptions, command: Command) -> CommandResult {
    match command {
        // Canal e LoL
        Command::Ping | Command::GetState => CommandResult::Ok,
        Command::LaunchLeague => launch_league(launch).await,
        Command::LaunchUx => hub.launch_ux().await,

        // Lobby, fila e partida encontrada
        Command::CreateLobby { queue } => hub.create_lobby(queue).await,
        Command::CreateLobbyById { queue_id } => hub.create_lobby_by_id(queue_id).await,
        Command::LeaveLobby => hub.leave_lobby().await,
        Command::SetPositions { first, second } => hub.set_positions(first, second).await,
        Command::StartQueue => hub.start_queue().await,
        Command::StopQueue => hub.stop_queue().await,
        Command::AcceptReadyCheck => hub.accept_ready_check().await,
        Command::DeclineReadyCheck => hub.decline_ready_check().await,
        Command::StartTutorial { queue_id } => hub.start_tutorial(queue_id).await,
        Command::StartPracticeTool => hub.start_practice_tool().await,
        Command::CreateCustomGame { options } => hub.create_custom_game(options).await,
        Command::SetPlayerSlots { slots } => hub.set_player_slots(slots).await,
        Command::SetLobbyOpen { open } => hub.set_lobby_open(open).await,
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
