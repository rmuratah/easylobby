//! Histórico de PDL do jogador logado (#457), para o gráfico de elo do
//! perfil.
//!
//! A LCU só sabe o rank de agora: nem ela nem a API da Riot guardam o PDL
//! de antes. Então o agente grava: cada vez que os ranks chegam (ao conectar
//! e no fim de cada ranqueada, veja [`crate::ranked`]) e mudaram desde o
//! último ponto, entra um ponto com a data, o tier, a divisão e o PDL.
//!
//! O histórico só existe para quem usa o app, a partir do dia em que
//! instalou, e só do jogador logado. Fica num arquivo JSON por conta
//! (`puuid`), em `lp-history/` dentro da pasta de configuração do easylobby
//! ([`crate::riot_api::config_dir`]).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ranked::{RankedEntry, RankedStats};

/// Pasta dos históricos, dentro da pasta de configuração.
pub const DIR: &str = "lp-history";
/// Pontos guardados por fila; os mais velhos saem primeiro. Dá alguns anos
/// de ranqueadas e o arquivo continua pequeno.
pub const MAX_POINTS: usize = 2000;

/// PDL do jogador logado ao longo do tempo, por fila, do ponto mais velho
/// para o mais novo.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
pub struct LpHistory {
    pub solo_duo: Vec<LpPoint>,
    pub flex: Vec<LpPoint>,
}

/// O rank numa fila num momento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LpPoint {
    /// Quando o agente viu esse rank, em milissegundos desde 1970 (UTC).
    #[ts(type = "number")]
    pub at: u64,
    /// Tier em maiúsculas, como em [`RankedEntry::tier`].
    pub tier: String,
    /// Divisão (`I` a `IV`), `null` de Mestre para cima.
    pub division: Option<String>,
    pub league_points: u32,
    pub wins: u32,
    pub losses: u32,
}

impl LpPoint {
    fn from_entry(entry: &RankedEntry, at: u64) -> Option<Self> {
        Some(LpPoint {
            at,
            tier: entry.tier.clone()?,
            division: entry.division.clone(),
            league_points: entry.league_points,
            wins: entry.wins,
            losses: entry.losses,
        })
    }

    /// Mesmo rank e mesmas partidas: nada a gravar.
    fn same_rank(&self, other: &LpPoint) -> bool {
        self.tier == other.tier
            && self.division == other.division
            && self.league_points == other.league_points
            && self.wins == other.wins
            && self.losses == other.losses
    }
}

impl LpHistory {
    /// Acrescenta um ponto em cada fila cujo rank mudou desde o último
    /// ponto. Fila sem classificação não entra. Devolve se algo mudou.
    pub fn record(&mut self, stats: &RankedStats, at: u64) -> bool {
        let solo = push(&mut self.solo_duo, &stats.solo_duo, at);
        let flex = push(&mut self.flex, &stats.flex, at);
        solo || flex
    }
}

fn push(points: &mut Vec<LpPoint>, entry: &RankedEntry, at: u64) -> bool {
    let Some(point) = LpPoint::from_entry(entry, at) else {
        return false;
    };
    if points.last().is_some_and(|last| last.same_rank(&point)) {
        return false;
    }
    points.push(point);
    if points.len() > MAX_POINTS {
        let extra = points.len() - MAX_POINTS;
        points.drain(..extra);
    }
    true
}

/// Arquivo do histórico da conta `puuid` dentro de `dir`. Caracteres fora
/// de letras, números e `-` saem do nome.
pub fn file_path(dir: &Path, puuid: &str) -> PathBuf {
    let name: String = puuid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    dir.join(DIR).join(format!("{name}.json"))
}

/// Lê o histórico de `puuid`. Sem arquivo, ou com um arquivo que não dá
/// para ler, começa vazio.
pub fn load(dir: &Path, puuid: &str) -> LpHistory {
    fs::read(file_path(dir, puuid))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Grava o histórico de `puuid`, num arquivo temporário trocado de uma vez
/// para não deixar um arquivo pela metade se o app fechar no meio.
pub fn save(dir: &Path, puuid: &str, history: &LpHistory) -> io::Result<()> {
    let path = file_path(dir, puuid);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(history).map_err(io::Error::other)?)?;
    fs::rename(&tmp, &path)
}

/// Agora, em milissegundos desde 1970.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(tier: &str, division: Option<&str>, lp: u32, wins: u32, losses: u32) -> RankedEntry {
        RankedEntry {
            tier: Some(tier.to_owned()),
            division: division.map(str::to_owned),
            league_points: lp,
            wins,
            losses,
            provisional: false,
        }
    }

    fn stats(solo: RankedEntry, flex: RankedEntry) -> RankedStats {
        RankedStats {
            solo_duo: solo,
            flex,
        }
    }

    #[test]
    fn grava_so_quando_o_rank_muda() {
        let mut history = LpHistory::default();
        let first = stats(entry("GOLD", Some("II"), 40, 10, 8), RankedEntry::default());
        assert!(history.record(&first, 1));
        assert!(!history.record(&first, 2), "mesmo rank não repete ponto");
        assert_eq!(history.solo_duo.len(), 1);
        assert!(history.flex.is_empty(), "fila sem classificação fica fora");

        let after_win = stats(
            entry("GOLD", Some("II"), 61, 11, 8),
            entry("SILVER", Some("I"), 10, 1, 0),
        );
        assert!(history.record(&after_win, 3));
        assert_eq!(history.solo_duo.len(), 2);
        assert_eq!(history.solo_duo[1].league_points, 61);
        assert_eq!(history.solo_duo[1].at, 3);
        assert_eq!(history.flex.len(), 1);
    }

    #[test]
    fn partida_sem_mudar_o_pdl_tambem_entra() {
        // Derrota com escudo de divisão: o PDL fica em 0, mas as derrotas sobem.
        let mut history = LpHistory::default();
        history.record(
            &stats(entry("GOLD", Some("IV"), 0, 5, 5), RankedEntry::default()),
            1,
        );
        assert!(history.record(
            &stats(entry("GOLD", Some("IV"), 0, 5, 6), RankedEntry::default()),
            2
        ));
    }

    #[test]
    fn limita_os_pontos() {
        let mut history = LpHistory::default();
        for i in 0..(MAX_POINTS as u32 + 5) {
            history.record(
                &stats(
                    entry("GOLD", Some("I"), i % 100, i, 0),
                    RankedEntry::default(),
                ),
                i as u64,
            );
        }
        assert_eq!(history.solo_duo.len(), MAX_POINTS);
        assert_eq!(history.solo_duo[0].at, 5, "os mais velhos saem primeiro");
    }

    #[test]
    fn grava_e_le_o_arquivo() {
        let dir = std::env::temp_dir().join(format!("easylobby-lp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            load(&dir, "abc"),
            LpHistory::default(),
            "sem arquivo começa vazio"
        );

        let mut history = LpHistory::default();
        history.record(
            &stats(entry("MASTER", None, 120, 50, 40), RankedEntry::default()),
            7,
        );
        save(&dir, "abc-123", &history).unwrap();
        assert_eq!(load(&dir, "abc-123"), history);

        fs::write(file_path(&dir, "ruim"), b"{nao e json").unwrap();
        assert_eq!(load(&dir, "ruim"), LpHistory::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nome_do_arquivo_sem_caminho() {
        let path = file_path(Path::new("base"), "../x/y");
        assert_eq!(path, Path::new("base").join(DIR).join("xy.json"));
    }
}
