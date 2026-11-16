//! Skin na seleção de campeões (#477).
//!
//! - A skin marcada do jogador logado já chega em `selectedSkinId` de
//!   [`ChampSelectPlayer`](crate::champ_select::ChampSelectPlayer).
//! - As skins do campeão (com `owned`) vêm do comando `openChampionSkins`
//!   (#369), em `championSkins`.
//! - Para trocar, o agente manda
//!   `PATCH /lol-champ-select/v1/session/my-selection` com
//!   `{"selectedSkinId": N}`, como o client oficial. A LCU responde sem corpo
//!   e a sessão nova chega pelo WAMP.
//!
//! - Croma (#316): o mesmo `selectedSkinId`, com o `id` do croma no lugar
//!   do da skin. Vale só croma que a conta tem, de uma skin que ela tem.
//!
//! [`check_skin`] confere o pedido antes de falar com a LCU.

use std::fmt;

use serde_json::{json, Value};

use crate::champ_select::{ChampSelect, ChampSelectPhase};
use crate::collection::ChampionSkins;
use crate::rest::{LcuClient, LcuError};
use crate::summoner_spells::MY_SELECTION_PATH;

/// Por que um pedido de `selectSkin` foi recusado sem falar com a LCU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidSkin {
    /// Sem seleção de campeões aberta (ou sem o jogador logado nela).
    NoChampSelect,
    /// A partida já está abrindo.
    SelectionOver,
    /// O jogador ainda não tem campeão.
    NoChampion,
    /// A skin não é do campeão do jogador.
    OtherChampion(u32),
    /// As skins do campeão ainda não foram pedidas com `openChampionSkins`.
    SkinsNotLoaded,
    /// A skin não existe nesse campeão.
    UnknownSkin(u32),
    /// A conta não tem a skin.
    NotOwned(u32),
    /// A conta não tem o croma.
    ChromaNotOwned(u32),
}

impl fmt::Display for InvalidSkin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoChampSelect => f.write_str("sem seleção de campeões aberta"),
            Self::SelectionOver => f.write_str("a partida já está abrindo"),
            Self::NoChampion => f.write_str("escolha o campeão antes da skin"),
            Self::OtherChampion(id) => write!(f, "a skin {id} não é do seu campeão"),
            Self::SkinsNotLoaded => f.write_str("as skins do campeão ainda não carregaram"),
            Self::UnknownSkin(id) => write!(f, "skin {id} não existe nesse campeão"),
            Self::NotOwned(id) => write!(f, "você não tem a skin {id}"),
            Self::ChromaNotOwned(id) => write!(f, "você não tem o croma {id}"),
        }
    }
}

/// Confere um pedido de skin (ou de croma, #316) contra a seleção atual e
/// as skins do campeão.
pub fn check_skin(
    champ_select: Option<&ChampSelect>,
    skins: Option<&ChampionSkins>,
    skin_id: u32,
) -> Result<(), InvalidSkin> {
    let cs = champ_select.ok_or(InvalidSkin::NoChampSelect)?;
    let me = cs.local_player().ok_or(InvalidSkin::NoChampSelect)?;
    if cs.timer.phase == ChampSelectPhase::GameStarting {
        return Err(InvalidSkin::SelectionOver);
    }
    let champion = me.champion_id.ok_or(InvalidSkin::NoChampion)?;
    // Os ids de skin são `campeão * 1000 + n`.
    if skin_id / 1000 != champion {
        return Err(InvalidSkin::OtherChampion(skin_id));
    }
    let skins = skins
        .filter(|s| s.champion_id == champion)
        .ok_or(InvalidSkin::SkinsNotLoaded)?;
    if let Some(skin) = skins.skins.iter().find(|s| s.id == skin_id) {
        return if skin.owned {
            Ok(())
        } else {
            Err(InvalidSkin::NotOwned(skin_id))
        };
    }
    // Um croma: precisa da skin dele e do próprio croma na conta.
    let (skin, chroma) = skins
        .skins
        .iter()
        .find_map(|s| s.chromas.iter().find(|c| c.id == skin_id).map(|c| (s, c)))
        .ok_or(InvalidSkin::UnknownSkin(skin_id))?;
    if !skin.owned {
        return Err(InvalidSkin::NotOwned(skin.id));
    }
    if !chroma.owned {
        return Err(InvalidSkin::ChromaNotOwned(skin_id));
    }
    Ok(())
}

/// Troca a skin do jogador logado. A LCU responde sem corpo; a sessão nova
/// chega pelo WAMP.
pub async fn set_skin(lcu: &LcuClient, skin_id: u32) -> Result<(), LcuError> {
    lcu.patch::<_, Option<Value>>(MY_SELECTION_PATH, &json!({ "selectedSkinId": skin_id }))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::{CollectionSkin, SkinChroma};

    fn session(phase: &str, champion: Option<u32>) -> ChampSelect {
        let raw = json!({
            "id": "s",
            "queueId": 420,
            "localPlayerCellId": 0,
            "timer": {"phase": phase},
            "myTeam": [{"cellId": 0, "championId": champion.unwrap_or(0)}],
        });
        ChampSelect::from_json(&raw).unwrap()
    }

    fn skin(id: u32, owned: bool) -> CollectionSkin {
        CollectionSkin {
            id,
            name: format!("Skin {id}"),
            is_base: id.is_multiple_of(1000),
            owned,
            rarity: None,
            chromas: Vec::new(),
        }
    }

    fn ahri_skins() -> ChampionSkins {
        ChampionSkins {
            champion_id: 103,
            skins: vec![
                skin(103_000, true),
                skin(103_001, true),
                skin(103_002, false),
            ],
        }
    }

    #[test]
    fn aceita_skin_da_conta_no_campeao_do_jogador() {
        let cs = session("BAN_PICK", Some(103));
        let skins = ahri_skins();
        assert_eq!(check_skin(Some(&cs), Some(&skins), 103_001), Ok(()));
        assert_eq!(check_skin(Some(&cs), Some(&skins), 103_000), Ok(()));
        let cs = session("FINALIZATION", Some(103));
        assert_eq!(check_skin(Some(&cs), Some(&skins), 103_001), Ok(()));
    }

    #[test]
    fn recusa_fora_da_selecao_ou_sem_campeao() {
        let skins = ahri_skins();
        assert_eq!(
            check_skin(None, Some(&skins), 103_001),
            Err(InvalidSkin::NoChampSelect)
        );
        let cs = session("GAME_STARTING", Some(103));
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_001),
            Err(InvalidSkin::SelectionOver)
        );
        let cs = session("BAN_PICK", None);
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_001),
            Err(InvalidSkin::NoChampion)
        );
    }

    #[test]
    fn recusa_skin_de_outro_campeao_desconhecida_ou_que_a_conta_nao_tem() {
        let cs = session("BAN_PICK", Some(103));
        let skins = ahri_skins();
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 266_001),
            Err(InvalidSkin::OtherChampion(266_001))
        );
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_009),
            Err(InvalidSkin::UnknownSkin(103_009))
        );
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_002),
            Err(InvalidSkin::NotOwned(103_002))
        );
    }

    fn chroma(id: u32, owned: bool) -> SkinChroma {
        SkinChroma {
            id,
            name: format!("Croma {id}"),
            owned,
            colors: vec!["#c82b2b".into()],
        }
    }

    #[test]
    fn aceita_so_croma_que_a_conta_tem_de_skin_que_ela_tem() {
        let cs = session("FINALIZATION", Some(103));
        let mut skins = ahri_skins();
        skins.skins[1].chromas = vec![chroma(103_020, true), chroma(103_021, false)];
        skins.skins[2].chromas = vec![chroma(103_030, true)];
        assert_eq!(check_skin(Some(&cs), Some(&skins), 103_020), Ok(()));
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_021),
            Err(InvalidSkin::ChromaNotOwned(103_021))
        );
        // Croma de uma skin que a conta não tem.
        assert_eq!(
            check_skin(Some(&cs), Some(&skins), 103_030),
            Err(InvalidSkin::NotOwned(103_002))
        );
    }

    #[test]
    fn recusa_sem_as_skins_do_campeao() {
        let cs = session("BAN_PICK", Some(103));
        assert_eq!(
            check_skin(Some(&cs), None, 103_001),
            Err(InvalidSkin::SkinsNotLoaded)
        );
        let other = ChampionSkins {
            champion_id: 266,
            skins: vec![skin(266_000, true)],
        };
        assert_eq!(
            check_skin(Some(&cs), Some(&other), 103_001),
            Err(InvalidSkin::SkinsNotLoaded)
        );
    }
}
