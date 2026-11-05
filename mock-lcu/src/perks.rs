//! Páginas de runas da LCU falsa (#26): criar, editar, apagar e aplicar.
//!
//! Como no lobby (#17), o mock parte das páginas gravadas e manda os mesmos
//! eventos que a LCU manda: a lista inteira em [`PAGES`], a página em uso em
//! [`CURRENT_PAGE`] e o espaço livre em [`INVENTORY`]. As árvores e os nomes
//! das runas ([`PERK_STYLES`] e [`PERKS`]) saem da gravação quando ela tiver;
//! senão, dos arquivos sintéticos em `mock-lcu/data/`.
//!
//! O mock dá [`DEFAULT_OWNED_PAGES`] páginas compradas até um evento
//! gravado de [`INVENTORY`] dizer outro número. Páginas temporárias (a
//! recomendada da seleção) não ocupam espaço. Uma troca de fase do gameflow
//! volta as páginas para as gravadas naquela fase, como acontece com o lobby.
//!
//! As funções daqui não mexem no estado: devolvem a resposta e os eventos, e
//! quem chama emite os eventos (o que atualiza o estado e avisa o WAMP).

use easylobby_agent::fixtures::EventFixture;
use serde_json::{json, Value};

use crate::state::{LcuState, Response};

/// Lista de páginas; `POST` cria.
pub const PAGES: &str = "/lol-perks/v1/pages";

/// Página em uso; `PUT` com o `id` no corpo aplica outra.
pub const CURRENT_PAGE: &str = "/lol-perks/v1/currentpage";

/// Páginas compradas e se cabe mais uma.
pub const INVENTORY: &str = "/lol-perks/v1/inventory";

/// Árvores de runas.
pub const PERK_STYLES: &str = "/lol-game-data/assets/v1/perkstyles.json";

/// Nomes e descrições das runas.
pub const PERKS: &str = "/lol-game-data/assets/v1/perks.json";

/// Páginas compradas quando a gravação não diz.
pub const DEFAULT_OWNED_PAGES: u64 = 5;

const SYNTHETIC_PERK_STYLES: &str = include_str!("../data/perkstyles.json");
const SYNTHETIC_PERKS: &str = include_str!("../data/perks.json");

/// `GET` dos arquivos de dados do jogo: o gravado, ou o sintético.
pub fn game_data(state: &LcuState, path: &str) -> Option<Response> {
    let synthetic = match path {
        PERK_STYLES => SYNTHETIC_PERK_STYLES,
        PERKS => SYNTHETIC_PERKS,
        _ => return None,
    };
    let recorded = state.get(path);
    if recorded.status == 200 {
        return Some(recorded);
    }
    Some(Response {
        status: 200,
        body: serde_json::from_str(synthetic).expect("dados sintéticos são JSON válido"),
    })
}

/// `GET /lol-perks/v1/inventory`, calculado com as páginas atuais.
pub fn inventory(state: &LcuState) -> Response {
    Response {
        status: 200,
        body: inventory_for(state, &pages(state)),
    }
}

fn inventory_for(state: &LcuState, list: &[Value]) -> Value {
    let owned = owned_pages(state);
    let custom = list.iter().filter(|p| p["isTemporary"] != true).count() as u64;
    json!({
        "canAddCustomPage": custom < owned,
        "customPageCount": custom,
        "isCustomPageCreationUnlocked": true,
        "ownedPageCount": owned,
    })
}

/// `GET /lol-perks/v1/pages/<id>`.
pub fn get_page(state: &LcuState, id: u64) -> Response {
    match pages(state).into_iter().find(|p| p["id"] == id) {
        Some(page) => Response {
            status: 200,
            body: page,
        },
        None => not_found(id),
    }
}

/// `POST /lol-perks/v1/pages`: cria a página e a aplica se `current` vier
/// `true`.
pub fn create(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    if inventory(state).body["canAddCustomPage"] != true {
        return (Response::error(400, "Max pages reached"), Vec::new());
    }
    let mut list = pages(state);
    let id = list
        .iter()
        .filter_map(|p| p["id"].as_u64())
        .max()
        .unwrap_or(0)
        + 1;
    let current = body["current"] == true;
    let mut page = json!({
        "autoModifiedSelections": [],
        "current": false,
        "id": id,
        "isActive": false,
        "isDeletable": true,
        "isEditable": true,
        "isTemporary": false,
        "isValid": true,
        "name": "",
        "order": 0,
        "primaryStyleId": 0,
        "subStyleId": 0,
        "selectedPerkIds": [],
        "uiPerks": [],
    });
    merge(&mut page, body);
    page["id"] = json!(id);
    page["current"] = json!(false);
    list.insert(0, page);
    if current {
        set_current(&mut list, id);
    }
    let created = list[0].clone();
    (
        Response {
            status: 200,
            body: created,
        },
        changes(state, list),
    )
}

/// `PUT /lol-perks/v1/pages/<id>`: troca os campos da página.
pub fn update(state: &LcuState, id: u64, body: &Value) -> (Response, Vec<EventFixture>) {
    let mut list = pages(state);
    let Some(page) = list.iter_mut().find(|p| p["id"] == id) else {
        return (not_found(id), Vec::new());
    };
    if page["isEditable"] == false {
        return (
            Response::error(400, format!("a página {id} não pode ser editada")),
            Vec::new(),
        );
    }
    let current = page["current"].clone();
    merge(page, body);
    page["id"] = json!(id);
    page["current"] = current;
    let updated = page.clone();
    (
        Response {
            status: 200,
            body: updated,
        },
        changes(state, list),
    )
}

/// `DELETE /lol-perks/v1/pages/<id>`. Se era a página em uso, a primeira que
/// sobrou passa a ser.
pub fn delete(state: &LcuState, id: u64) -> (Response, Vec<EventFixture>) {
    let mut list = pages(state);
    let Some(index) = list.iter().position(|p| p["id"] == id) else {
        return (not_found(id), Vec::new());
    };
    if list[index]["isDeletable"] == false {
        return (
            Response::error(400, format!("a página {id} não pode ser apagada")),
            Vec::new(),
        );
    }
    let removed = list.remove(index);
    if removed["current"] == true {
        if let Some(first) = list.first().and_then(|p| p["id"].as_u64()) {
            set_current(&mut list, first);
        }
    }
    (no_content(), changes(state, list))
}

/// `PUT /lol-perks/v1/currentpage` com o `id` no corpo.
pub fn select(state: &LcuState, body: &Value) -> (Response, Vec<EventFixture>) {
    let Some(id) = body.as_u64() else {
        return (
            Response::error(400, "mock-lcu: o corpo precisa ser o id da página"),
            Vec::new(),
        );
    };
    let mut list = pages(state);
    if !list.iter().any(|p| p["id"] == id) {
        return (not_found(id), Vec::new());
    }
    set_current(&mut list, id);
    (no_content(), changes(state, list))
}

/// `id` de `/lol-perks/v1/pages/<id>`.
pub fn page_id(path: &str) -> Option<u64> {
    path.strip_prefix(PAGES)?.strip_prefix('/')?.parse().ok()
}

fn pages(state: &LcuState) -> Vec<Value> {
    let response = state.get(PAGES);
    match response.body {
        Value::Array(list) if response.status == 200 => list,
        _ => Vec::new(),
    }
}

fn owned_pages(state: &LcuState) -> u64 {
    let recorded = state.get(INVENTORY);
    if recorded.status == 200 {
        if let Some(owned) = recorded.body["ownedPageCount"].as_u64() {
            return owned;
        }
    }
    DEFAULT_OWNED_PAGES
}

fn set_current(list: &mut [Value], id: u64) {
    for page in list {
        let current = page["id"] == id;
        page["current"] = json!(current);
        page["isActive"] = json!(current);
    }
}

/// Copia para `page` os campos do corpo que o cliente mandou.
fn merge(page: &mut Value, body: &Value) {
    let (Some(page), Some(body)) = (page.as_object_mut(), body.as_object()) else {
        return;
    };
    for (key, value) in body {
        page.insert(key.clone(), value.clone());
    }
}

/// Eventos da lista nova: as páginas, a página em uso e o inventário, nessa
/// ordem, como a LCU.
fn changes(state: &LcuState, list: Vec<Value>) -> Vec<EventFixture> {
    let current = list.iter().find(|p| p["current"] == true).cloned();
    let inventory = inventory_for(state, &list);
    let mut events = vec![event(PAGES, Value::Array(list))];
    if let Some(current) = current {
        events.push(event(CURRENT_PAGE, current));
    }
    events.push(event(INVENTORY, inventory));
    events
}

fn not_found(id: u64) -> Response {
    Response::error(404, format!("a página {id} não existe"))
}

fn no_content() -> Response {
    Response {
        status: 204,
        body: Value::Null,
    }
}

fn event(uri: &str, data: Value) -> EventFixture {
    EventFixture {
        t_ms: 0,
        uri: uri.into(),
        event_type: "Update".into(),
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use easylobby_agent::fixtures::FixtureSet;

    fn state() -> LcuState {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
        LcuState::new(&FixtureSet::load(&dir).unwrap())
    }

    fn apply(state: &mut LcuState, events: Vec<EventFixture>) {
        for e in &events {
            state.apply(e);
        }
    }

    fn ids(state: &LcuState) -> Vec<u64> {
        pages(state)
            .iter()
            .filter_map(|p| p["id"].as_u64())
            .collect()
    }

    fn current(state: &LcuState) -> u64 {
        state.get(CURRENT_PAGE).body["id"].as_u64().unwrap()
    }

    #[test]
    fn dados_do_jogo_sinteticos() {
        let s = state();
        let styles = game_data(&s, PERK_STYLES).unwrap();
        assert_eq!(styles.body["styles"].as_array().unwrap().len(), 5);
        let perks = game_data(&s, PERKS).unwrap();
        assert!(perks.body.as_array().unwrap().len() > 60);
        assert!(game_data(&s, PAGES).is_none());
    }

    #[test]
    fn inventario_calculado() {
        let s = state();
        let inv = inventory(&s).body;
        assert_eq!(inv["customPageCount"], 3);
        assert_eq!(inv["ownedPageCount"], DEFAULT_OWNED_PAGES);
        assert_eq!(inv["canAddCustomPage"], true);
    }

    #[test]
    fn cria_aplica_edita_e_apaga() {
        let mut s = state();
        let body = json!({
            "name": "Nova",
            "primaryStyleId": 8100,
            "subStyleId": 8000,
            "selectedPerkIds": [8112, 8126, 8137, 8135, 9111, 8017, 5008, 5008, 5011],
            "current": true,
        });
        let (response, events) = create(&s, &body);
        assert_eq!(response.status, 200);
        let id = response.body["id"].as_u64().unwrap();
        assert_eq!(events[0].uri, PAGES);
        apply(&mut s, events);
        assert_eq!(ids(&s)[0], id);
        assert_eq!(current(&s), id);
        assert_eq!(get_page(&s, id).body["name"], "Nova");

        let (response, events) = update(&s, id, &json!({"name": "Renomeada"}));
        assert_eq!(response.status, 200);
        apply(&mut s, events);
        assert_eq!(get_page(&s, id).body["name"], "Renomeada");
        assert_eq!(get_page(&s, id).body["primaryStyleId"], 8100);
        assert_eq!(current(&s), id);

        let (response, events) = select(&s, &json!(156_827_123u64));
        assert_eq!(response.status, 204);
        apply(&mut s, events);
        assert_eq!(current(&s), 156_827_123);

        let (response, events) = delete(&s, 156_827_123);
        assert_eq!(response.status, 204);
        apply(&mut s, events);
        assert!(!ids(&s).contains(&156_827_123));
        // Apagou a em uso: a primeira que sobrou passa a ser.
        assert_eq!(current(&s), id);

        assert_eq!(delete(&s, 1).0.status, 404);
        assert_eq!(update(&s, 1, &json!({})).0.status, 404);
        assert_eq!(select(&s, &json!(1)).0.status, 404);
        assert_eq!(select(&s, &json!("x")).0.status, 400);
    }

    #[test]
    fn sem_espaco_nao_cria() {
        let mut s = state();
        s.apply(&event(
            INVENTORY,
            json!({"ownedPageCount": 3, "canAddCustomPage": false}),
        ));
        let (response, events) = create(&s, &json!({"name": "x"}));
        assert_eq!(response.status, 400);
        assert!(events.is_empty());
    }

    #[test]
    fn caminho_da_pagina() {
        assert_eq!(page_id("/lol-perks/v1/pages/42"), Some(42));
        assert_eq!(page_id("/lol-perks/v1/pages"), None);
        assert_eq!(page_id("/lol-perks/v1/pages/x"), None);
    }
}
