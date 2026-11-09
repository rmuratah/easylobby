//! Criar partida personalizada com as opções do client oficial (#262).
//!
//! A personalizada nasce no mesmo `POST /lol-lobby/v2/lobby` do Modo de
//! Treino ([`crate::training`]), com `customGameLobby`:
//!
//! ```json
//! {
//!   "customGameLobby": {
//!     "configuration": {
//!       "gameMode": "CLASSIC", "gameMutator": "", "gameServerRegion": "",
//!       "mapId": 11, "mutators": { "id": 18 },
//!       "spectatorPolicy": "AllAllowed", "teamSize": 5
//!     },
//!     "lobbyName": "Partida de Fulano", "lobbyPassword": ""
//!   },
//!   "isCustom": true
//! }
//! ```
//!
//! **`queueId` obrigatório (#330):** no LoL 16.22 o corpo sem `queueId`
//! volta 500 `INVALID_LOBBY`, com qualquer opção (testado no LoL real). O
//! agente manda a fila personalizada do mapa e do tipo de seleção
//! ([`CustomGameOptions::queue_id`]); quem decide o tipo é ela, e o
//! `mutators.id` só acompanha.
//!
//! O tipo de seleção vai em `mutators.id`: é o `gameTypeConfig.id` das filas
//! personalizadas da lista da LCU (gravação real do patch 16.22): 19 às
//! cegas, 18 alternada, 21 todos aleatórios e 6 alternada para torneios.
//! Nessa lista, os dois mapas aceitam os quatro tipos (filas 3100 a 3130 no
//! Summoner's Rift e 3200 a 3230 no Abismo Uivante), com até 5 por equipe.
//! Por isso a UI pode oferecer qualquer combinação dos tipos abaixo, e o
//! agente só confere o nome e o tamanho da equipe.
//!
//! O Rift Clássico (#270) é o modo `JADE` no mapa 453 e só tem as filas
//! personalizadas 3260 (às cegas) e 3262 (alternada): os outros tipos de
//! seleção respondem `invalidCommand`.
//!
//! "Esconder da lista de saguões públicos" e "Intervalo de espectador" (#270)
//! vão em `configuration.isPrivate` e em
//! `gameCustomization.spectatorGridDelayEnabled`. O segundo é o nome que a LCU
//! mostra em `/lol-lobby/v1/parties/gamemode` (gravação do #271), com o valor
//! em texto (`"true"` ou `"false"`); o primeiro ainda precisa ser conferido no
//! LoL real.
//!
//! O Mapa aleatório do client oficial (#271) é o Abismo Uivante (mapa 12) com
//! a "Seleção de mapas" em `gameCustomization.aramMapMutator`: `NONE` no
//! Howling Abyss, `MapSkin_Map12_Bloom` na Travessia de Koeshin e
//! `MapSkin_HA_Bilgewater` na Ponte da Carnificina ([`AramMap`]). O "Mapa
//! aleatório" da lista não tem valor próprio: o client sorteia um dos três, e o
//! agente faz o mesmo. O ARAM: Desordem é o modo `KIWI` no mesmo mapa (fila
//! 3270, `mutators.id` 19), como mostram as gravações do LoL real.
//!
//! Depois de criada, a personalizada mostra no lobby o nome, o mapa, o tipo de
//! seleção e quem pode assistir ([`CustomGameSettings`]), lidos do
//! `gameConfig` (`customLobbyName`, `mapId`, `gameMode`,
//! `customMutatorName`, `customSpectatorPolicy`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::lobby::{Lobby, LOBBY_PATH};
use crate::rest::{LcuClient, LcuError};

/// Maior equipe de uma personalizada.
pub const MAX_TEAM_SIZE: u32 = 5;

/// Maior nome de personalizada aceito pelo agente, em caracteres.
pub const MAX_NAME_LEN: usize = 64;

/// Mapa e modo da personalizada, como o client oficial oferece. O Modo de
/// Treino fica fora: ele continua só no card da aba Treinamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CustomMap {
    /// Summoner's Rift, modo Clássico (`CLASSIC`, mapa 11).
    SummonersRift,
    /// Abismo Uivante, modo ARAM (`ARAM`, mapa 12). É o modo "Mapa aleatório"
    /// do client oficial (#271), com a seleção de mapas em [`AramMap`].
    HowlingAbyss,
    /// Rift Clássico (`JADE`, mapa 453, #270): só às cegas e alternada.
    ClassicRift,
}

impl CustomMap {
    pub fn map_id(self) -> u32 {
        match self {
            CustomMap::SummonersRift => 11,
            CustomMap::HowlingAbyss => 12,
            CustomMap::ClassicRift => 453,
        }
    }

    pub fn game_mode(self) -> &'static str {
        match self {
            CustomMap::SummonersRift => "CLASSIC",
            CustomMap::HowlingAbyss => "ARAM",
            CustomMap::ClassicRift => "JADE",
        }
    }

    /// Se o mapa aceita o tipo de seleção, pelas filas personalizadas da LCU.
    pub fn allows(self, pick_type: CustomPickType) -> bool {
        match self {
            CustomMap::ClassicRift => matches!(
                pick_type,
                CustomPickType::BlindPick | CustomPickType::DraftMode
            ),
            CustomMap::SummonersRift => pick_type != CustomPickType::AramMayhem,
            CustomMap::HowlingAbyss => true,
        }
    }

    /// O mapa de uma personalizada pelo `mapId` do `gameConfig`. `None` em
    /// outros mapas (Arena, TFT...).
    pub fn from_map_id(map_id: u64) -> Option<CustomMap> {
        match map_id {
            11 => Some(CustomMap::SummonersRift),
            12 => Some(CustomMap::HowlingAbyss),
            453 => Some(CustomMap::ClassicRift),
            _ => None,
        }
    }
}

/// Tipo de seleção de campeões da personalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CustomPickType {
    /// Escolha às cegas.
    BlindPick,
    /// Escolha alternada, com bans.
    DraftMode,
    /// Todos aleatórios.
    AllRandom,
    /// Alternada para torneios.
    TournamentDraft,
    /// ARAM: Desordem (#271), só no Abismo Uivante: o modo `KIWI`.
    AramMayhem,
}

impl CustomPickType {
    /// `mutators.id` do corpo de criar: o `gameTypeConfig.id` das filas
    /// personalizadas da LCU.
    pub fn mutator_id(self) -> u32 {
        match self {
            CustomPickType::BlindPick => 19,
            CustomPickType::DraftMode => 18,
            CustomPickType::AllRandom => 21,
            CustomPickType::TournamentDraft => 6,
            CustomPickType::AramMayhem => 19,
        }
    }

    /// O tipo pelo `customMutatorName` do `gameConfig`. `None` com um nome
    /// desconhecido (as filas comuns usam `TeamBuilderDraftPickStrategy`).
    pub fn from_mutator_name(name: &str) -> Option<CustomPickType> {
        match name {
            "SimulPickStrategy" | "BlindPickStrategy" => Some(CustomPickType::BlindPick),
            "DraftModeSinglePickStrategy" | "DraftPickStrategy" => Some(CustomPickType::DraftMode),
            "AllRandomPickStrategy" => Some(CustomPickType::AllRandom),
            "TournamentPickStrategy" | "TournamentDraftPickStrategy" => {
                Some(CustomPickType::TournamentDraft)
            }
            _ => None,
        }
    }
}

/// "Seleção de mapas" do Mapa aleatório (#271): a variante do Abismo Uivante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AramMap {
    /// Howling Abyss (`NONE`).
    HowlingAbyss,
    /// Travessia de Koeshin (`MapSkin_Map12_Bloom`).
    KoeshinCrossing,
    /// Ponte da Carnificina (`MapSkin_HA_Bilgewater`).
    ButchersBridge,
    /// Mapa aleatório: o agente sorteia um dos três, como o client oficial.
    Random,
}

impl AramMap {
    const CHOICES: [AramMap; 3] = [
        AramMap::HowlingAbyss,
        AramMap::KoeshinCrossing,
        AramMap::ButchersBridge,
    ];

    /// Valor de `gameCustomization.aramMapMutator`. O `Random` sorteia.
    pub fn to_lcu(self) -> &'static str {
        match self {
            AramMap::HowlingAbyss => "NONE",
            AramMap::KoeshinCrossing => "MapSkin_Map12_Bloom",
            AramMap::ButchersBridge => "MapSkin_HA_Bilgewater",
            AramMap::Random => Self::random().to_lcu(),
        }
    }

    /// Um dos três mapas. O sorteio não precisa ser criptográfico: usa os
    /// nanossegundos do relógio.
    fn random() -> AramMap {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        Self::CHOICES[nanos as usize % Self::CHOICES.len()]
    }
}

/// Quem pode assistir à personalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SpectatorPolicy {
    /// Ninguém (`NotAllowed`).
    None,
    /// Só quem está no lobby (`LobbyAllowed`).
    LobbyOnly,
    /// Só amigos (`FriendsAllowed`).
    FriendsOnly,
    /// Todos (`AllAllowed`).
    All,
}

impl SpectatorPolicy {
    pub fn to_lcu(self) -> &'static str {
        match self {
            SpectatorPolicy::None => "NotAllowed",
            SpectatorPolicy::LobbyOnly => "LobbyAllowed",
            SpectatorPolicy::FriendsOnly => "FriendsAllowed",
            SpectatorPolicy::All => "AllAllowed",
        }
    }

    pub fn from_lcu(value: &str) -> Option<SpectatorPolicy> {
        match value {
            "NotAllowed" => Some(SpectatorPolicy::None),
            "LobbyAllowed" => Some(SpectatorPolicy::LobbyOnly),
            "FriendsAllowed" => Some(SpectatorPolicy::FriendsOnly),
            "AllAllowed" => Some(SpectatorPolicy::All),
            _ => None,
        }
    }
}

/// As opções do formulário de criar personalizada (comando
/// `createCustomGame`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameOptions {
    /// Nome da partida, como aparece na lista de personalizadas. Não pode
    /// ficar vazio nem passar de 64 caracteres. O client oficial sugere
    /// "Partida de {invocador}".
    pub name: String,
    /// Senha para entrar. Vazia deixa a partida sem senha.
    pub password: String,
    pub map: CustomMap,
    pub pick_type: CustomPickType,
    /// Seleção de mapas do Mapa aleatório (#271). Só vale no Abismo Uivante;
    /// nos outros mapas é ignorada.
    pub aram_map: AramMap,
    /// Jogadores por equipe, de 1 a 5.
    pub team_size: u32,
    pub spectators: SpectatorPolicy,
    /// "Esconder da lista de saguões públicos" (#270).
    pub hide_from_public_list: bool,
    /// "Intervalo de espectador" (#270): quem assiste vê a partida com
    /// atraso. O client oficial deixa ligado.
    pub spectator_delay: bool,
}

impl CustomGameOptions {
    /// Confere as opções antes de falar com a LCU. Devolve o motivo do erro.
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("o nome da personalizada não pode ficar vazio".into());
        }
        if name.chars().count() > MAX_NAME_LEN {
            return Err(format!(
                "o nome da personalizada passa de {MAX_NAME_LEN} caracteres"
            ));
        }
        if !self.map.allows(self.pick_type) {
            return Err(format!(
                "o mapa {:?} não aceita o tipo de seleção {:?}",
                self.map, self.pick_type
            ));
        }
        if !(1..=MAX_TEAM_SIZE).contains(&self.team_size) {
            return Err(format!(
                "o tamanho da equipe vai de 1 a {MAX_TEAM_SIZE}, veio {}",
                self.team_size
            ));
        }
        Ok(())
    }

    /// `gameMode` da LCU: o do mapa, ou `KIWI` no ARAM: Desordem.
    pub fn game_mode(&self) -> &'static str {
        match self.pick_type {
            CustomPickType::AramMayhem => "KIWI",
            _ => self.map.game_mode(),
        }
    }

    /// Fila personalizada do mapa e do tipo de seleção, na lista da LCU do
    /// 16.22 (#330). Combinações que o mapa não aceita (barradas no
    /// [`CustomGameOptions::validate`]) caem na fila às cegas do mapa.
    pub fn queue_id(&self) -> u32 {
        use CustomPickType::*;
        match (self.map, self.pick_type) {
            (CustomMap::SummonersRift, DraftMode) => 3110,
            (CustomMap::SummonersRift, AllRandom) => 3120,
            (CustomMap::SummonersRift, TournamentDraft) => 3130,
            (CustomMap::SummonersRift, _) => 3100,
            (CustomMap::HowlingAbyss, DraftMode) => 3210,
            (CustomMap::HowlingAbyss, AllRandom) => 3220,
            (CustomMap::HowlingAbyss, TournamentDraft) => 3230,
            (CustomMap::HowlingAbyss, AramMayhem) => 3270,
            (CustomMap::HowlingAbyss, BlindPick) => 3200,
            (CustomMap::ClassicRift, DraftMode) => 3262,
            (CustomMap::ClassicRift, _) => 3260,
        }
    }

    /// Corpo do `POST /lol-lobby/v2/lobby` que cria a personalizada.
    pub fn body(&self) -> Value {
        lobby_body(&LobbyConfig {
            queue_id: self.queue_id(),
            name: self.name.trim(),
            password: &self.password,
            game_mode: self.game_mode(),
            map_id: self.map.map_id(),
            mutator_id: self.pick_type.mutator_id(),
            team_size: self.team_size,
            spectators: self.spectators,
            extras: Some(Extras {
                private: self.hide_from_public_list,
                spectator_delay: self.spectator_delay,
                aram_map: (self.map == CustomMap::HowlingAbyss).then(|| self.aram_map.to_lcu()),
            }),
        })
    }
}

/// Tudo o que vai no `customGameLobby`. Usado também pelo Modo de Treino.
pub(crate) struct LobbyConfig<'a> {
    /// Fila personalizada (#330): sem ela a LCU responde `INVALID_LOBBY`.
    pub queue_id: u32,
    pub name: &'a str,
    pub password: &'a str,
    pub game_mode: &'a str,
    pub map_id: u32,
    pub mutator_id: u32,
    pub team_size: u32,
    pub spectators: SpectatorPolicy,
    /// Esconder da lista e intervalo de espectador (#270). `None` não manda
    /// os campos (o Modo de Treino segue com o corpo do #209).
    pub extras: Option<Extras>,
}

/// Opções do #270 e do #271, que vão no `configuration` e no
/// `gameCustomization`.
pub(crate) struct Extras {
    pub private: bool,
    pub spectator_delay: bool,
    /// `aramMapMutator`, só no Abismo Uivante.
    pub aram_map: Option<&'static str>,
}

/// Corpo do `POST /lol-lobby/v2/lobby` de uma personalizada.
pub(crate) fn lobby_body(config: &LobbyConfig) -> Value {
    let mut body = json!({
        "customGameLobby": {
            "configuration": {
                "gameMode": config.game_mode,
                "gameMutator": "",
                "gameServerRegion": "",
                "mapId": config.map_id,
                "mutators": { "id": config.mutator_id },
                "spectatorPolicy": config.spectators.to_lcu(),
                "teamSize": config.team_size
            },
            "lobbyName": config.name,
            "lobbyPassword": config.password
        },
        "isCustom": true,
        "queueId": config.queue_id
    });
    if let Some(extras) = &config.extras {
        body["customGameLobby"]["configuration"]["isPrivate"] = json!(extras.private);
        body["gameCustomization"] = json!({
            "spectatorGridDelayEnabled": extras.spectator_delay.to_string()
        });
        if let Some(aram_map) = extras.aram_map {
            body["gameCustomization"]["aramMapMutator"] = json!(aram_map);
        }
    }
    body
}

/// Cria a personalizada. Devolve o lobby que a LCU respondeu.
pub async fn create(
    lcu: &LcuClient,
    options: &CustomGameOptions,
) -> Result<Option<Lobby>, LcuError> {
    let body: Value = lcu.post(LOBBY_PATH, &options.body()).await?;
    Ok(Lobby::from_json(&body))
}

/// As opções de uma personalizada já criada, para o cabeçalho do lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CustomGameSettings {
    /// Nome da partida (`customLobbyName`).
    pub name: String,
    /// `gameMode` da LCU: `CLASSIC`, `ARAM`, `PRACTICETOOL`...
    pub game_mode: String,
    /// `mapId` da LCU: 11 no Summoner's Rift, 12 no Abismo Uivante.
    pub map_id: u32,
    /// O mapa, ou `null` fora dos dois que o formulário oferece.
    pub map: Option<CustomMap>,
    /// O tipo de seleção, ou `null` quando a LCU manda um nome desconhecido.
    pub pick_type: Option<CustomPickType>,
    /// Quem pode assistir, ou `null` quando a LCU manda um valor
    /// desconhecido.
    pub spectators: Option<SpectatorPolicy>,
}

impl CustomGameSettings {
    /// Lê as opções do `gameConfig` de uma personalizada.
    pub fn from_game_config(config: &Value) -> CustomGameSettings {
        let text = |key: &str| config.get(key).and_then(Value::as_str).unwrap_or_default();
        let map_id = config.get("mapId").and_then(Value::as_u64).unwrap_or(0);
        CustomGameSettings {
            name: text("customLobbyName").to_owned(),
            game_mode: text("gameMode").to_owned(),
            map_id: u32::try_from(map_id).unwrap_or(0),
            map: CustomMap::from_map_id(map_id),
            // O ARAM: Desordem manda AllRandomPickStrategy: o modo diz mais.
            pick_type: if text("gameMode") == "KIWI" {
                Some(CustomPickType::AramMayhem)
            } else {
                CustomPickType::from_mutator_name(text("customMutatorName"))
            },
            spectators: SpectatorPolicy::from_lcu(text("customSpectatorPolicy")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> CustomGameOptions {
        CustomGameOptions {
            name: "  Partida de Fulano ".into(),
            password: "segredo".into(),
            map: CustomMap::HowlingAbyss,
            pick_type: CustomPickType::DraftMode,
            aram_map: AramMap::KoeshinCrossing,
            team_size: 3,
            spectators: SpectatorPolicy::FriendsOnly,
            hide_from_public_list: true,
            spectator_delay: false,
        }
    }

    #[test]
    fn corpo_leva_as_opcoes_do_formulario() {
        let body = options().body();
        let config = &body["customGameLobby"]["configuration"];
        assert_eq!(config["gameMode"], "ARAM");
        assert_eq!(config["mapId"], 12);
        assert_eq!(config["mutators"]["id"], 18);
        assert_eq!(config["teamSize"], 3);
        assert_eq!(config["spectatorPolicy"], "FriendsAllowed");
        assert_eq!(config["isPrivate"], true);
        assert_eq!(
            body["gameCustomization"]["spectatorGridDelayEnabled"],
            "false"
        );
        assert_eq!(
            body["gameCustomization"]["aramMapMutator"],
            "MapSkin_Map12_Bloom"
        );
        assert_eq!(body["customGameLobby"]["lobbyName"], "Partida de Fulano");
        assert_eq!(body["customGameLobby"]["lobbyPassword"], "segredo");
        assert_eq!(body["isCustom"], true);
        assert_eq!(body["queueId"], 3210);
    }

    #[test]
    fn fila_personalizada_de_cada_mapa_e_tipo() {
        let queue = |map, pick_type| {
            CustomGameOptions {
                map,
                pick_type,
                ..options()
            }
            .queue_id()
        };
        use CustomMap::*;
        use CustomPickType::*;
        assert_eq!(queue(SummonersRift, BlindPick), 3100);
        assert_eq!(queue(SummonersRift, DraftMode), 3110);
        assert_eq!(queue(SummonersRift, AllRandom), 3120);
        assert_eq!(queue(SummonersRift, TournamentDraft), 3130);
        assert_eq!(queue(HowlingAbyss, BlindPick), 3200);
        assert_eq!(queue(HowlingAbyss, DraftMode), 3210);
        assert_eq!(queue(HowlingAbyss, AllRandom), 3220);
        assert_eq!(queue(HowlingAbyss, TournamentDraft), 3230);
        assert_eq!(queue(HowlingAbyss, AramMayhem), 3270);
        assert_eq!(queue(ClassicRift, BlindPick), 3260);
        assert_eq!(queue(ClassicRift, DraftMode), 3262);
    }

    #[test]
    fn mapa_aleatorio_manda_a_selecao_de_mapas_e_a_desordem_vira_kiwi() {
        let mayhem = CustomGameOptions {
            pick_type: CustomPickType::AramMayhem,
            aram_map: AramMap::ButchersBridge,
            ..options()
        };
        assert_eq!(mayhem.validate(), Ok(()));
        let body = mayhem.body();
        assert_eq!(body["customGameLobby"]["configuration"]["gameMode"], "KIWI");
        assert_eq!(body["customGameLobby"]["configuration"]["mapId"], 12);
        assert_eq!(
            body["gameCustomization"]["aramMapMutator"],
            "MapSkin_HA_Bilgewater"
        );

        let random = CustomGameOptions {
            aram_map: AramMap::Random,
            ..options()
        };
        let chosen = random.body()["gameCustomization"]["aramMapMutator"].clone();
        assert!(
            ["NONE", "MapSkin_Map12_Bloom", "MapSkin_HA_Bilgewater"]
                .contains(&chosen.as_str().unwrap()),
            "{chosen}"
        );

        // Fora do Abismo Uivante, a seleção de mapas não vai e a Desordem não vale.
        let rift = CustomGameOptions {
            map: CustomMap::SummonersRift,
            ..options()
        };
        assert!(rift.body()["gameCustomization"]
            .get("aramMapMutator")
            .is_none());
        let rift_mayhem = CustomGameOptions {
            pick_type: CustomPickType::AramMayhem,
            ..rift
        };
        assert!(rift_mayhem.validate().is_err());

        let settings = CustomGameSettings::from_game_config(&json!({
            "gameMode": "KIWI", "mapId": 12, "customMutatorName": "AllRandomPickStrategy"
        }));
        assert_eq!(settings.pick_type, Some(CustomPickType::AramMayhem));
    }

    #[test]
    fn tipos_de_selecao_usam_o_game_type_config_das_filas() {
        // Filas personalizadas do Summoner's Rift na gravação do 16.22.
        let rest: Value =
            serde_json::from_str(include_str!("../tests/fixtures/16.22/rest.json")).unwrap();
        let queues = rest
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == "/lol-game-queues/v1/queues")
            .map(|e| e["body"].clone())
            .unwrap();
        let game_type = |id: u64| {
            queues
                .as_array()
                .unwrap()
                .iter()
                .find(|q| q["id"] == id)
                .map(|q| q["gameTypeConfig"]["id"].as_u64().unwrap())
                .unwrap()
        };
        for (queue, pick) in [
            (3100, CustomPickType::BlindPick),
            (3110, CustomPickType::DraftMode),
            (3120, CustomPickType::AllRandom),
            (3130, CustomPickType::TournamentDraft),
            (3200, CustomPickType::BlindPick),
            (3210, CustomPickType::DraftMode),
            (3220, CustomPickType::AllRandom),
            (3230, CustomPickType::TournamentDraft),
            (3270, CustomPickType::AramMayhem),
        ] {
            assert_eq!(
                game_type(queue),
                u64::from(pick.mutator_id()),
                "fila {queue}"
            );
        }
    }

    #[test]
    fn valida_nome_e_tamanho_da_equipe() {
        assert_eq!(options().validate(), Ok(()));
        for broken in [
            CustomGameOptions {
                name: "   ".into(),
                ..options()
            },
            CustomGameOptions {
                name: "x".repeat(MAX_NAME_LEN + 1),
                ..options()
            },
            CustomGameOptions {
                team_size: 0,
                ..options()
            },
            CustomGameOptions {
                team_size: 6,
                ..options()
            },
            CustomGameOptions {
                map: CustomMap::ClassicRift,
                pick_type: CustomPickType::AllRandom,
                ..options()
            },
        ] {
            assert!(broken.validate().is_err(), "{broken:?}");
        }
        let one = CustomGameOptions {
            team_size: 1,
            password: String::new(),
            ..options()
        };
        assert_eq!(one.validate(), Ok(()));
        let classic = CustomGameOptions {
            map: CustomMap::ClassicRift,
            pick_type: CustomPickType::BlindPick,
            ..options()
        };
        assert_eq!(classic.validate(), Ok(()));
        let body = classic.body();
        assert_eq!(body["customGameLobby"]["configuration"]["gameMode"], "JADE");
        assert_eq!(body["customGameLobby"]["configuration"]["mapId"], 453);
    }

    #[test]
    fn le_as_opcoes_do_lobby_gravado() {
        // Personalizada de ARAM: Desordem gravada no LoL real (16.22-aram).
        let config = json!({
            "customLobbyName": "Partida de fake-name-0001",
            "customMutatorName": "AllRandomPickStrategy",
            "customSpectatorPolicy": "AllAllowed",
            "gameMode": "KIWI",
            "isCustom": true,
            "mapId": 12
        });
        assert_eq!(
            CustomGameSettings::from_game_config(&config),
            CustomGameSettings {
                name: "Partida de fake-name-0001".into(),
                game_mode: "KIWI".into(),
                map_id: 12,
                map: Some(CustomMap::HowlingAbyss),
                pick_type: Some(CustomPickType::AramMayhem),
                spectators: Some(SpectatorPolicy::All),
            }
        );
        let unknown = CustomGameSettings::from_game_config(&json!({
            "customMutatorName": "TeamBuilderDraftPickStrategy",
            "customSpectatorPolicy": "?",
            "mapId": 30
        }));
        assert_eq!(unknown.map, None);
        assert_eq!(unknown.pick_type, None);
        assert_eq!(unknown.spectators, None);
    }
}
