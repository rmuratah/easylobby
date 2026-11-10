//! Lobby de personalizada (#229): bots, troca de equipe e iniciar a partida.

use super::{not_connected, Hub};
use crate::custom_game::SpectatorPolicy;
use crate::custom_lobby::{self, BotDifficulty, CustomBot, CustomLobby, CustomTeam};
use crate::lobby::Position;
use crate::rest::{LcuClient, LcuError};
use crate::training;
use crate::ws::protocol::{CommandResult, ErrorCode, ServerMessage};

/// As 5 rotas que um bot pode ter: sem preenchimento nem "sem posição".
const BOT_POSITIONS: [Position; 5] = [
    Position::Top,
    Position::Jungle,
    Position::Middle,
    Position::Bottom,
    Position::Utility,
];

/// O que um comando de personalizada precisa: a conexão, as equipes e o
/// `puuid` do jogador logado.
struct Context {
    client: Option<LcuClient>,
    custom: CustomLobby,
    me: String,
}

impl Context {
    /// A conexão com o LoL, ou `notConnected`.
    fn client(&self) -> Result<&LcuClient, CommandResult> {
        self.client.as_ref().ok_or_else(not_connected)
    }
}

fn invalid(message: impl Into<String>) -> CommandResult {
    CommandResult::error(ErrorCode::InvalidCommand, message)
}

fn check_bot(
    champion_id: u32,
    difficulty: BotDifficulty,
    position: Position,
) -> Result<(), CommandResult> {
    if champion_id == 0 {
        return Err(invalid("escolha o campeão do bot"));
    }
    if difficulty == BotDifficulty::Unknown {
        return Err(invalid("escolha a dificuldade do bot"));
    }
    if !BOT_POSITIONS.contains(&position) {
        return Err(invalid("o bot precisa de uma das 5 rotas"));
    }
    Ok(())
}

impl Hub {
    /// Confere a personalizada antes de falar com a LCU. Fora de
    /// personalizada responde `invalidCommand`. A conexão é conferida depois
    /// das regras, por [`Context::client`].
    fn custom_context(&self, leader_only: bool) -> Result<Context, CommandResult> {
        let shared = self.lock();
        let lobby = shared.state.lobby.as_ref();
        let Some(custom) = lobby.and_then(|l| l.custom.clone()) else {
            return Err(invalid("fora de uma personalizada"));
        };
        let is_leader = lobby.is_some_and(|l| l.is_leader);
        if leader_only && !is_leader {
            return Err(invalid("só o líder mexe nos bots e inicia a partida"));
        }
        let client = shared.client.clone();
        let me = shared
            .state
            .summoner
            .as_ref()
            .map(|s| s.puuid.clone())
            .unwrap_or_default();
        Ok(Context { client, custom, me })
    }

    /// Relê o lobby depois de um comando que deu certo.
    async fn after_custom(&self, result: Result<(), LcuError>) -> CommandResult {
        match result {
            Ok(()) => {
                self.refresh_lobby().await;
                CommandResult::Ok
            }
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Adiciona um bot na equipe `team` (comando `addBot`).
    pub async fn add_bot(
        &self,
        team: CustomTeam,
        champion_id: u32,
        difficulty: BotDifficulty,
        position: Position,
    ) -> CommandResult {
        if let Err(error) = check_bot(champion_id, difficulty, position) {
            return error;
        }
        let context = match self.custom_context(true) {
            Ok(context) => context,
            Err(error) => return error,
        };
        if context.custom.team(team).len() >= context.custom.team_size as usize {
            return invalid("a equipe está cheia");
        }
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        let result = custom_lobby::add_bot(client, team, champion_id, difficulty, position).await;
        self.after_custom(result).await
    }

    /// Muda o campeão, a dificuldade e a posição do bot `uuid` (comando
    /// `updateBot`).
    pub async fn update_bot(
        &self,
        uuid: &str,
        champion_id: u32,
        difficulty: BotDifficulty,
        position: Position,
    ) -> CommandResult {
        if let Err(error) = check_bot(champion_id, difficulty, position) {
            return error;
        }
        let context = match self.custom_context(true) {
            Ok(context) => context,
            Err(error) => return error,
        };
        let Some((team, bot)) = find_bot(&context.custom, uuid) else {
            return invalid("esse bot não está no lobby");
        };
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        let result =
            custom_lobby::update_bot(client, &bot, team, champion_id, difficulty, position).await;
        self.after_custom(result).await
    }

    /// Tira o bot `uuid` (comando `removeBot`).
    pub async fn remove_bot(&self, uuid: &str) -> CommandResult {
        let context = match self.custom_context(true) {
            Ok(context) => context,
            Err(error) => return error,
        };
        let Some((team, bot)) = find_bot(&context.custom, uuid) else {
            return invalid("esse bot não está no lobby");
        };
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        let result = custom_lobby::remove_bot(client, &bot, team).await;
        self.after_custom(result).await
    }

    /// Leva o jogador logado para a equipe `team` (comando `switchTeam`, o
    /// "Juntar-se").
    pub async fn switch_team(&self, team: CustomTeam) -> CommandResult {
        let context = match self.custom_context(false) {
            Ok(context) => context,
            Err(error) => return error,
        };
        if context.custom.team_of(&context.me) == Some(team) {
            return invalid("você já está nessa equipe");
        }
        if context.custom.team(team).len() >= context.custom.team_size as usize {
            return invalid("a equipe está cheia");
        }
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        let result = custom_lobby::switch_team(client, team).await;
        self.after_custom(result).await
    }

    /// Leva o jogador logado para os espectadores (comando `spectate`, #263).
    pub async fn spectate(&self) -> CommandResult {
        let context = match self.custom_context(false) {
            Ok(context) => context,
            Err(error) => return error,
        };
        if context.custom.settings.spectators == Some(SpectatorPolicy::None) {
            return invalid("a personalizada não aceita espectadores");
        }
        if context.custom.is_spectator(&context.me) {
            return invalid("você já está assistindo");
        }
        if !context.custom.has_other_player(&context.me) {
            return invalid("só dá para assistir com outro jogador na personalizada");
        }
        if context.custom.spectators.len() >= context.custom.max_spectators as usize {
            return invalid("as vagas de espectador estão cheias");
        }
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        let result = custom_lobby::spectate(client).await;
        self.after_custom(result).await
    }

    /// Inicia a partida da personalizada (comando `startCustomGame`): a
    /// seleção de campeões chega pelo WAMP.
    pub async fn start_custom_game(&self) -> CommandResult {
        let context = match self.custom_context(true) {
            Ok(context) => context,
            Err(error) => return error,
        };
        let client = match context.client() {
            Ok(client) => client,
            Err(error) => return error,
        };
        match training::start_custom_champ_select(client).await {
            Ok(()) => CommandResult::Ok,
            Err(error) => CommandResult::error(ErrorCode::Lcu, error.to_string()),
        }
    }

    /// Publica os campeões que podem ser bots (#229).
    pub fn set_available_bots(&self, bots: Option<Vec<u32>>) {
        self.update(|shared| {
            if shared.state.available_bots == bots {
                return None;
            }
            shared.state.available_bots = bots.clone();
            Some(ServerMessage::AvailableBots {
                available_bots: bots,
            })
        });
    }

    /// Lê em segundo plano os campeões que podem ser bots, quando há uma
    /// personalizada e a lista ainda não veio nesta conexão.
    pub(super) fn spawn_refresh_available_bots(&self) {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let (client, session) = {
            let shared = self.lock();
            let custom = shared
                .state
                .lobby
                .as_ref()
                .is_some_and(|l| l.custom.is_some());
            if !custom || shared.state.available_bots.is_some() {
                return;
            }
            let Some(client) = shared.client.clone() else {
                return;
            };
            (client, shared.session)
        };
        let hub = self.clone();
        runtime.spawn(async move {
            if let Ok(Some(bots)) = custom_lobby::available_bots(&client).await {
                if hub.lock().session == session {
                    hub.set_available_bots(Some(bots));
                }
            }
        });
    }
}

fn find_bot(custom: &CustomLobby, uuid: &str) -> Option<(CustomTeam, CustomBot)> {
    custom.bot(uuid).map(|(team, bot)| (team, bot.clone()))
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;
    use crate::lobby::LOBBY_PATH;
    use crate::wamp::EventType;
    use serde_json::json;

    fn is_invalid(result: &CommandResult) -> bool {
        matches!(
            result,
            CommandResult::Error {
                code: ErrorCode::InvalidCommand,
                ..
            }
        )
    }

    fn custom_lobby(leader: bool, team1: usize) -> serde_json::Value {
        let mut members = vec![json!({"puuid": "eu", "isLeader": leader})];
        for i in 1..team1 {
            members.push(
                json!({"isBot": true, "botChampionId": i, "botUuid": format!("bot-{i}"),
                                "botDifficulty": "RSINTRO", "botPosition": "TOP"}),
            );
        }
        json!({
            "gameConfig": {"queueId": 3140, "isCustom": true, "maxTeamSize": 5,
                           "customTeam100": members, "customTeam200": []},
            "localMember": {"puuid": "eu", "isLeader": leader},
            "members": [{"puuid": "eu", "isLeader": leader}],
        })
    }

    #[tokio::test]
    async fn fora_de_personalizada_recusa_sem_falar_com_a_lcu() {
        let hub = Hub::new();
        for result in [
            hub.add_bot(CustomTeam::Team2, 22, BotDifficulty::Intro, Position::Top)
                .await,
            hub.remove_bot("x").await,
            hub.switch_team(CustomTeam::Team2).await,
            hub.spectate().await,
            hub.start_custom_game().await,
        ] {
            assert!(is_invalid(&result), "{result:?}");
        }
    }

    #[tokio::test]
    async fn confere_bot_lider_e_vagas() {
        let hub = Hub::new();
        hub.set_summoner(Some(crate::summoner::Summoner {
            game_name: "Eu".into(),
            tag_line: "BR1".into(),
            display_name: "Eu".into(),
            summoner_level: 30,
            profile_icon_id: 1,
            puuid: "eu".into(),
            summoner_id: 1,
        }));
        // Bot sem rota ou com preenchimento, sem campeão.
        for result in [
            hub.add_bot(CustomTeam::Team2, 22, BotDifficulty::Intro, Position::Fill)
                .await,
            hub.add_bot(CustomTeam::Team2, 0, BotDifficulty::Intro, Position::Top)
                .await,
            hub.add_bot(CustomTeam::Team2, 22, BotDifficulty::Unknown, Position::Top)
                .await,
        ] {
            assert!(is_invalid(&result), "{result:?}");
        }
        hub.apply(event(LOBBY_PATH, EventType::Create, custom_lobby(false, 1)))
            .await;
        assert!(hub.state().lobby.unwrap().custom.is_some());
        // Quem não é o líder não mexe nos bots nem inicia.
        assert!(is_invalid(
            &hub.add_bot(CustomTeam::Team2, 22, BotDifficulty::Intro, Position::Top)
                .await
        ));
        assert!(is_invalid(&hub.start_custom_game().await));
        // Mas troca de equipe: aqui o LoL não está conectado.
        assert!(matches!(
            hub.switch_team(CustomTeam::Team2).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        assert!(is_invalid(&hub.switch_team(CustomTeam::Team1).await));

        hub.apply(event(LOBBY_PATH, EventType::Update, custom_lobby(true, 5)))
            .await;
        assert!(is_invalid(
            &hub.add_bot(CustomTeam::Team1, 22, BotDifficulty::Intro, Position::Top)
                .await
        ));
        assert!(is_invalid(&hub.remove_bot("nao-existe").await));
        assert!(matches!(
            hub.remove_bot("bot-2").await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn confere_politica_e_vagas_de_espectador() {
        let hub = Hub::new();
        hub.set_summoner(Some(crate::summoner::Summoner {
            game_name: "Eu".into(),
            tag_line: "BR1".into(),
            display_name: "Eu".into(),
            summoner_level: 30,
            profile_icon_id: 1,
            puuid: "eu".into(),
            summoner_id: 1,
        }));
        let with = |policy: &str, spectators: serde_json::Value| {
            let mut lobby = custom_lobby(false, 1);
            lobby["gameConfig"]["customSpectatorPolicy"] = json!(policy);
            lobby["gameConfig"]["maxLobbySpectatorCount"] = json!(1);
            lobby["gameConfig"]["customSpectators"] = spectators;
            lobby
        };
        hub.apply(event(
            LOBBY_PATH,
            EventType::Create,
            with("NotAllowed", json!([])),
        ))
        .await;
        assert!(is_invalid(&hub.spectate().await));
        hub.apply(event(
            LOBBY_PATH,
            EventType::Update,
            with("AllAllowed", json!([{"puuid": "outro"}])),
        ))
        .await;
        assert!(is_invalid(&hub.spectate().await), "vagas cheias");
        let mut watching = with("AllAllowed", json!([{"puuid": "eu"}]));
        watching["gameConfig"]["customTeam100"] = json!([]);
        hub.apply(event(LOBBY_PATH, EventType::Update, watching))
            .await;
        assert!(is_invalid(&hub.spectate().await), "já assistindo");
        // Assistindo, volta pelo "Juntar-se": aqui o LoL não está conectado.
        assert!(matches!(
            hub.switch_team(CustomTeam::Team1).await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
        // Sozinho (com bots), a LCU não deixa assistir (#409).
        hub.apply(event(
            LOBBY_PATH,
            EventType::Update,
            with("LobbyAllowed", json!([])),
        ))
        .await;
        assert!(is_invalid(&hub.spectate().await), "sem outro jogador");
        let mut with_friend = with("LobbyAllowed", json!([]));
        with_friend["gameConfig"]["customTeam200"] = json!([{"puuid": "amigo"}]);
        hub.apply(event(LOBBY_PATH, EventType::Update, with_friend))
            .await;
        assert!(matches!(
            hub.spectate().await,
            CommandResult::Error {
                code: ErrorCode::NotConnected,
                ..
            }
        ));
    }
}
