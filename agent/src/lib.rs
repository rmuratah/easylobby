//! Agente do easylobby.
//!
//! Acha o processo do LoL, lê o lockfile, conecta na LCU API (REST + WAMP) e
//! expõe um canal `/ws` tipado para a UI. Módulos:
//!
//! - [`lockfile`]: acha o LoL e lê porta e senha da LCU (#4)
//! - [`fixtures`]: formato e sanitização das fixtures gravadas da LCU (#5)
//! - [`rest`]: cliente REST da LCU, com Basic Auth e certificado fixado (#8)
//! - [`summoner`]: invocador logado (#7)
//! - [`wamp`]: eventos da LCU em tempo real pelo WebSocket WAMP (#9)
//! - [`connection`]: conexão REST + WAMP que reconecta quando o LoL fecha ou
//!   reinicia (#10)

pub mod connection;
pub mod fixtures;
pub mod lockfile;
pub mod rest;
pub mod summoner;
pub mod wamp;

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
