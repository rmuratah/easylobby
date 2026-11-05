//! Mostra onde o agente achou as credenciais da LCU, sem imprimir a senha.
//!
//! ```bash
//! cargo run -p easylobby-agent --example descobrir_lcu
//! ```

use easylobby_agent::lockfile;

fn main() {
    match lockfile::discover() {
        Ok(credentials) => {
            println!("LCU em {}", credentials.base_url());
            println!("{credentials:?}");
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
