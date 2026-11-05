//! Agente do easylobby.
//!
//! Acha o processo do LoL, lê o lockfile, conecta na LCU API (REST + WAMP) e
//! expõe um canal `/ws` tipado para a UI. Módulos:
//!
//! - [`lockfile`]: acha o LoL e lê porta e senha da LCU (#4)
//!
//! Próximas issues da Fase 0: #8 cliente REST da LCU, #9 cliente WebSocket WAMP.

pub mod lockfile;

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
