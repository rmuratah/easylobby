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
//! - [`matchmaking`]: entrar e sair da fila, com o tempo na fila (#19)
//! - [`ws`]: canal `/ws` com eventos e comandos tipados para a UI (#15)
//! - [`gameflow`]: fase do gameflow (lobby, fila, ready check, champ
//!   select, em jogo, pós-jogo) (#16)
//! - [`ready_check`]: partida encontrada, com aceitar e recusar (#20)

pub mod assets;
pub mod connection;
pub mod fixtures;
pub mod gameflow;
pub mod launcher;
pub mod lobby;
pub mod lockfile;
pub mod matchmaking;
pub mod ready_check;
pub mod rest;
pub mod summoner;
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
