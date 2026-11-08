//! Agente do easylobby.
//!
//! Acha o processo do LoL, lê o lockfile, conecta na LCU API (REST + WAMP) e
//! expõe um canal `/ws` tipado para a UI. Módulos:
//!
//! - [`assets`]: cache local de ícones e imagens, servido em `/assets` (#29)
//! - [`lockfile`]: acha o LoL e lê porta e senha da LCU (#4)
//! - [`fixtures`]: formato e sanitização das fixtures gravadas da LCU (#5)
//! - [`rest`]: cliente REST da LCU, com Basic Auth e certificado fixado (#8)
//! - [`summoner`]: invocador logado (#7)
//! - [`wamp`]: eventos da LCU em tempo real pelo WebSocket WAMP (#9)
//! - [`connection`]: conexão REST + WAMP que reconecta quando o LoL fecha ou
//!   reinicia (#10)
//! - [`launcher`]: abre o LoL pelo Riot Client quando ele não está rodando
//!   (#11)
//! - [`ux`]: fecha a UI oficial ao conectar (`kill-ux`) e reabre
//!   (`launch-ux`) (#12)
//! - [`lobby`]: lobby e escolha de fila (#17)
//! - [`queues`]: filas disponíveis para criar lobby, lidas da LCU (#146)
//! - [`training`]: Tutorial e Modo de Treino da aba Treinamento (#209)
//! - [`invitations`]: convites de lobby recebidos e enviados (#32)
//! - [`matchmaking`]: entrar e sair da fila, com o tempo na fila (#19)
//! - [`ws`]: canal `/ws` com eventos e comandos tipados para a UI (#15)
//! - [`gameflow`]: fase do gameflow (lobby, fila, ready check, champ
//!   select, em jogo, pós-jogo) (#16)
//! - [`ready_check`]: partida encontrada, com aceitar e recusar (#20)
//! - [`champ_select`]: estado da seleção de campeões (sessão, times, timer e
//!   ações pendentes) (#22)
//! - [`chat`]: conversas 1:1 com amigos e o histórico da sessão (#31)
//! - [`champ_select_chat`]: chat do time na seleção de campeões (#27)
//! - [`lobby_chat`]: chat do grupo no lobby (#206)
//! - [`champions`]: campeões do jogo e os que dá para escolher e banir na
//!   seleção (#28)
//! - [`runes`]: páginas de runas: listar, editar, criar, apagar e aplicar,
//!   com as árvores para o editor (#26)
//! - [`friends`]: lista de amigos com presença (#30)
//! - [`summoner_spells`]: feitiços de invocador na seleção de campeões (#24)
//! - [`ranked`]: ranks do jogador logado na Solo/Duo e na Flex (#171)
//! - [`match_history`]: histórico de partidas e placar de uma partida (#172,
//!   #174)
//! - [`mastery`]: campeões com mais maestria (#175)
//! - [`swaps`]: trocas de campeão, de ordem de pick e de cargo na seleção
//!   (#25)

pub mod assets;
pub mod bench;
pub mod champ_select;
pub mod champ_select_chat;
pub mod champions;
pub mod chat;
pub mod connection;
pub mod custom_lobby;
pub mod fixtures;
pub mod friends;
pub mod gameflow;
pub mod invitations;
pub mod launcher;
pub mod lobby;
pub mod lobby_chat;
pub mod lobby_link;
pub mod lobby_restrictions;
pub mod lockfile;
pub mod mastery;
pub mod match_history;
pub mod matchmaking;
pub mod player_profile;
pub mod players;
pub mod queues;
pub mod quick_play;
pub mod ranked;
pub mod ready_check;
pub mod rest;
pub mod runes;
pub mod summoner;
pub mod summoner_spells;
pub mod swaps;
pub mod training;
pub mod ux;
pub mod wamp;
pub mod ws;

/// Versão do agente, a mesma do `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_vem_do_cargo_toml() {
        assert!(!VERSION.is_empty());
    }
}
