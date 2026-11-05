//! mock-lcu: servidor falso da LCU para desenvolver sem abrir o LoL.
//!
//! Vai responder com as respostas gravadas em `agent/tests/fixtures/<patch>/`
//! e reproduzir os eventos WAMP. A implementação é a issue #6; as fixtures
//! são a issue #5.

fn main() {
    println!(
        "mock-lcu {}: ainda não implementado (issue #6)",
        env!("CARGO_PKG_VERSION")
    );
}
