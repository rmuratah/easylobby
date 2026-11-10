//! Notas da atualização atual do LoL, para o card do Início (#301).
//!
//! A LCU não tem as notas, só a versão do jogo em [`GAME_VERSION_PATH`]
//! (`"16.22.8348910+branch..."`). A versão `16.22` do jogo é a atualização
//! `26.22` do site (desde 2025 o nome usa o ano: `15.x` virou `25.x`). Com o
//! nome, o agente monta o endereço da página das notas no site oficial e lê
//! dela o título e o resumo, das tags `og:title` e `og:description` do
//! `<head>`, e as mudanças, dos títulos e parágrafos do corpo (#385, ver
//! [`parse_sections`]). Se a página não responder ou mudar de formato, as
//! notas saem só com a versão e o endereço da lista de notas, e o card
//! continua útil.
//!
//! A UI não acessa a internet (a CSP do app só libera o agente) e a janela do
//! Tauri não abre links externos; por isso quem busca a página e quem abre o
//! navegador ([`open_in_browser`]) é o agente.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::rest::LcuClient;

/// Versão completa do jogo, como `"16.22.8348910+branch.releases-16-22..."`.
pub const GAME_VERSION_PATH: &str = "/lol-patch/v1/game-version";

/// Site oficial do LoL em português, de onde saem as notas.
pub const DEFAULT_SITE: &str = "https://www.leagueoflegends.com/pt-br";

/// Lista de todas as notas, para quando a página da atualização não existe.
const NOTES_LIST_PATH: &str = "/news/tags/patch-notes/";

/// Só endereços do site oficial são abertos no navegador.
const ALLOWED_PREFIX: &str = "https://www.leagueoflegends.com/";

/// Limite da página das notas: o texto vem antes dos dados do Next.js no fim,
/// então uma página cortada aqui ainda tem quase todas as mudanças.
const MAX_PAGE_BYTES: usize = 6 * 1024 * 1024;

/// Limites das seções mandadas para a UI, para uma página estranha não virar
/// uma mensagem enorme no `/ws`.
const MAX_SECTIONS: usize = 30;
const MAX_CHANGES: usize = 80;
const MAX_LINES: usize = 60;
const MAX_TEXT_CHARS: usize = 600;

/// Seções do fim da página que não são notas.
const STOP_SECTIONS: [&str; 2] = ["artigos relacionados", "related articles"];

/// Tempo máximo para a página das notas responder.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// As notas da atualização em que o LoL está.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PatchNotes {
    /// Nome da atualização como o site mostra, como `"26.22"`.
    pub version: String,
    /// Título da página das notas, ou `null` se o site não respondeu.
    pub title: Option<String>,
    /// Resumo da atualização (o subtítulo das notas), ou `null`.
    pub summary: Option<String>,
    /// Página aberta pelo comando `openPatchNotes`: as notas da atualização,
    /// ou a lista de notas se o site não respondeu.
    pub url: String,
    /// Mudanças da atualização por seção (Campeões, Itens...), lidas da
    /// página das notas (#385). Vazio se o site não respondeu ou mudou de
    /// formato.
    pub sections: Vec<PatchSection>,
}

/// Uma seção das notas, como "Campeões" ou "Correções de bugs".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PatchSection {
    pub title: String,
    pub changes: Vec<PatchChange>,
}

/// Uma mudança dentro da seção, como um campeão ou um item. As linhas de
/// antes do primeiro título da seção ficam numa mudança sem título.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PatchChange {
    pub title: Option<String>,
    pub lines: Vec<PatchLine>,
}

/// Uma linha de texto da mudança.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PatchLine {
    pub kind: PatchLineKind,
    pub text: String,
}

/// `heading`: subtítulo (uma habilidade, por exemplo); `text`: parágrafo;
/// `bullet`: item de lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum PatchLineKind {
    Heading,
    Text,
    Bullet,
}

/// `"16.22.8348910+branch..."` -> `"26.22"`. Recusa versões de antes de
/// 2025 (`14.x` e anteriores), que tinham outro nome no site, e o que não
/// for número.
pub fn patch_name(game_version: &str) -> Option<String> {
    let mut parts = game_version.split(['.', '+']);
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    (major >= 15).then(|| format!("{}.{minor:02}", major + 10))
}

/// Página das notas da atualização `name` (`"26.22"`) no `site`.
pub fn notes_url(site: &str, name: &str) -> String {
    format!(
        "{site}/news/game-updates/league-of-legends-patch-{}-notes/",
        name.replace('.', "-")
    )
}

/// Lê a versão do jogo na LCU e busca as notas no `site`. `None` só quando
/// a LCU não deu a versão; sem `site` (nos testes) ou com o site fora do ar,
/// as notas saem sem título e resumo, com a lista de notas no `url`.
pub async fn current(client: &LcuClient, site: Option<&str>) -> Option<PatchNotes> {
    let game_version: String = client.get(GAME_VERSION_PATH).await.ok()?;
    let version = patch_name(&game_version)?;
    let fallback = |site: &str| PatchNotes {
        version: version.clone(),
        title: None,
        summary: None,
        url: format!("{site}{NOTES_LIST_PATH}"),
        sections: Vec::new(),
    };
    let Some(site) = site else {
        return Some(fallback(DEFAULT_SITE));
    };
    let url = notes_url(site, &version);
    match fetch_page(&url).await {
        Some(page) => {
            let meta = parse_meta(&page);
            if meta.title.is_none() && meta.description.is_none() {
                return Some(fallback(site));
            }
            Some(PatchNotes {
                version,
                title: meta.title,
                summary: meta.description,
                url,
                sections: parse_sections(&page),
            })
        }
        None => Some(fallback(site)),
    }
}

/// A página (até [`MAX_PAGE_BYTES`]), ou `None` se não respondeu com
/// sucesso.
async fn fetch_page(url: &str) -> Option<String> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("easylobby/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .ok()?;
    let mut response = http.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() >= MAX_PAGE_BYTES {
            bytes.truncate(MAX_PAGE_BYTES);
            break;
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// O que interessa das tags `<meta>` da página.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PageMeta {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Lê `og:title` e `og:description` (ou `description`) das tags `<meta>`,
/// com os atributos em qualquer ordem e aspas simples ou duplas.
pub fn parse_meta(html: &str) -> PageMeta {
    let mut meta = PageMeta::default();
    let mut plain_description = None;
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<meta") {
        let start = from + start;
        let Some(end) = lower[start..].find('>') else {
            break;
        };
        let end = start + end;
        let tag = &html[start..end];
        from = end;
        let key = attribute(tag, "property").or_else(|| attribute(tag, "name"));
        let Some(content) = attribute(tag, "content") else {
            continue;
        };
        let content = decode_entities(content.trim());
        if content.is_empty() {
            continue;
        }
        match key.map(str::to_ascii_lowercase).as_deref() {
            Some("og:title") if meta.title.is_none() => meta.title = Some(content),
            Some("og:description") if meta.description.is_none() => {
                meta.description = Some(content)
            }
            Some("description") if plain_description.is_none() => plain_description = Some(content),
            _ => {}
        }
    }
    meta.description = meta.description.or(plain_description);
    meta
}

/// Valor do atributo `name` na tag, sem as aspas.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        // O nome inteiro: `name` não pode casar com o fim de `og:site_name`.
        let before = lower[..at].chars().next_back();
        if !before.is_some_and(char::is_whitespace) {
            continue;
        }
        let rest = lower[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value_at = tag.len() - rest.len();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value = &tag[value_at + 1..];
        let close = value.find(quote)?;
        return Some(&value[..close]);
    }
    None
}

/// Troca as entidades HTML comuns (`&amp;`, `&quot;`, `&#39;`, `&#x2014;`...)
/// pelo caractere.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &rest[1..end];
            let ch = match entity {
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "nbsp" => Some('\u{a0}'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|ch| (ch, end))
        });
        match decoded {
            Some((ch, end)) => {
                out.push(ch);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Lê as mudanças do corpo da página das notas. Não depende das classes do
/// site, só da hierarquia dos títulos, que é a mesma há anos: `h2` abre uma
/// seção ("Campeões"), `h3` uma mudança ("Corki"), `h4` é um subtítulo (uma
/// habilidade), `p` um parágrafo e `li` um item de lista. Começa no título
/// do artigo (`h1`), para pular o menu, e para no rodapé ou em "Artigos
/// relacionados". Seções sem texto (só imagem, como os destaques) saem.
pub fn parse_sections(html: &str) -> Vec<PatchSection> {
    let lower = html.to_ascii_lowercase();
    let mut sections: Vec<PatchSection> = Vec::new();
    let mut pos = lower.find("<h1").unwrap_or(0);
    while let Some(found) = lower[pos..].find('<') {
        let at = pos + found;
        let name = tag_name(&lower[at + 1..]);
        let Some(close) = lower[at..].find('>') else {
            break;
        };
        let content = at + close + 1;
        pos = content;
        match name {
            "script" | "style" | "noscript" | "svg" | "template" => {
                match lower[content..].find(&format!("</{name}")) {
                    Some(end) => pos = content + end,
                    None => break,
                }
            }
            "footer" => break,
            "h2" | "h3" | "h4" | "p" | "li" => {
                let text = element_text(html, &lower, content);
                if text.is_empty() {
                    continue;
                }
                if name == "h2" {
                    if STOP_SECTIONS.contains(&text.to_lowercase().as_str()) {
                        break;
                    }
                    sections.push(PatchSection {
                        title: text,
                        changes: Vec::new(),
                    });
                    continue;
                }
                let Some(section) = sections.last_mut() else {
                    continue;
                };
                if name == "h3" {
                    section.changes.push(PatchChange {
                        title: Some(text),
                        lines: Vec::new(),
                    });
                    continue;
                }
                let kind = match name {
                    "h4" => PatchLineKind::Heading,
                    "li" => PatchLineKind::Bullet,
                    _ => PatchLineKind::Text,
                };
                if section.changes.is_empty() {
                    section.changes.push(PatchChange {
                        title: None,
                        lines: Vec::new(),
                    });
                }
                if let Some(change) = section.changes.last_mut() {
                    change.lines.push(PatchLine { kind, text });
                }
            }
            _ => {}
        }
    }
    sections.retain(|section| !section.changes.is_empty());
    sections.truncate(MAX_SECTIONS);
    for section in &mut sections {
        section.changes.truncate(MAX_CHANGES);
        for change in &mut section.changes {
            change.lines.truncate(MAX_LINES);
        }
    }
    sections
}

/// Nome da tag no começo de `rest` (logo depois do `<`): `"h2"` em
/// `"h2 id=..."`, vazio em `"/h2>"` e `"!--"`.
fn tag_name(rest: &str) -> &str {
    let len = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    &rest[..len]
}

/// Tags que terminam o texto de um elemento: o próprio fim dele ou o começo
/// de outro bloco (um `li` com uma lista dentro fica só com o texto de antes
/// da lista; a lista de dentro é lida depois, item a item).
const BLOCK_TAGS: [&str; 24] = [
    "p",
    "li",
    "ul",
    "ol",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "div",
    "blockquote",
    "table",
    "tr",
    "td",
    "th",
    "section",
    "article",
    "header",
    "footer",
    "figure",
    "figcaption",
    "script",
    "hr",
];

/// O texto do elemento que começa em `start` (logo depois da tag de
/// abertura), sem as tags de dentro, com as entidades trocadas e os espaços
/// juntados.
fn element_text(html: &str, lower: &str, start: usize) -> String {
    let mut raw = String::new();
    let mut pos = start;
    loop {
        let Some(found) = lower[pos..].find('<') else {
            raw.push_str(&html[pos..]);
            break;
        };
        let at = pos + found;
        raw.push_str(&html[pos..at]);
        let rest = &lower[at + 1..];
        let name = tag_name(rest.strip_prefix('/').unwrap_or(rest));
        if BLOCK_TAGS.contains(&name) {
            break;
        }
        if name == "br" {
            raw.push(' ');
        }
        let Some(close) = lower[at..].find('>') else {
            break;
        };
        pos = at + close + 1;
    }
    let text = decode_entities(&raw);
    let mut words = text.split_whitespace();
    let mut out = String::new();
    if let Some(first) = words.next() {
        out.push_str(first);
        for word in words {
            out.push(' ');
            out.push_str(word);
        }
    }
    if out.chars().count() > MAX_TEXT_CHARS {
        out = out.chars().take(MAX_TEXT_CHARS - 1).collect::<String>() + "…";
    }
    out
}

/// Se o endereço é do site oficial, o único que [`open_in_browser`] abre.
pub fn allowed_url(url: &str) -> bool {
    url.starts_with(ALLOWED_PREFIX)
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"')
}

/// Abre `url` no navegador padrão, sem esperar por ele.
pub fn open_in_browser(url: &str) -> std::io::Result<()> {
    if !allowed_url(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "endereço fora do site oficial do LoL",
        ));
    }
    #[cfg(windows)]
    let mut command = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_da_atualizacao_sai_da_versao_do_jogo() {
        let version = "16.22.8348910+branch.releases-16-22.code.public.content.release";
        assert_eq!(patch_name(version).as_deref(), Some("26.22"));
        assert_eq!(patch_name("16.3.1").as_deref(), Some("26.03"));
        assert_eq!(patch_name("15.10.1").as_deref(), Some("25.10"));
        assert_eq!(patch_name("14.24.1"), None);
        assert_eq!(patch_name("lolpatch_7.20"), None);
        assert_eq!(patch_name(""), None);
    }

    #[test]
    fn endereco_das_notas_segue_o_site() {
        assert_eq!(
            notes_url(DEFAULT_SITE, "26.22"),
            "https://www.leagueoflegends.com/pt-br/news/game-updates/league-of-legends-patch-26-22-notes/"
        );
    }

    #[test]
    fn le_titulo_e_resumo_das_tags_og() {
        let html = r#"<html><head>
            <meta charSet="utf-8"/>
            <meta property="og:site_name" content="League of Legends"/>
            <meta name="description" content="Resumo genérico"/>
            <meta content="Notas da Atualização 26.22 do League of Legends" property="og:title"/>
            <meta property='og:description' content='K/DA chegando pra botar fogo no parquinho &amp; mais &#x2014; &quot;Mel&quot;'>
            </head><body>"#;
        assert_eq!(
            parse_meta(html),
            PageMeta {
                title: Some("Notas da Atualização 26.22 do League of Legends".into()),
                description: Some(
                    "K/DA chegando pra botar fogo no parquinho & mais \u{2014} \"Mel\"".into()
                ),
            }
        );
    }

    #[test]
    fn sem_og_description_usa_a_description() {
        let html = r#"<meta name="description" content="Resumo"><meta name="og:title" content="">"#;
        assert_eq!(
            parse_meta(html),
            PageMeta {
                title: None,
                description: Some("Resumo".into()),
            }
        );
    }

    #[test]
    fn pagina_sem_tags_nao_quebra() {
        assert_eq!(parse_meta("<html><meta"), PageMeta::default());
        assert_eq!(parse_meta("&amp"), PageMeta::default());
        assert_eq!(decode_entities("a & b &zzz; &amp"), "a & b &zzz; &amp");
    }

    const NOTES_PAGE: &str = r#"<html><head><title>x</title></head><body>
        <nav><h2>Menu do site</h2><ul><li>Notícias</li></ul></nav>
        <h1>Notas da Atualização 26.22</h1>
        <div id="patch-notes-container">
          <header class="header-primary"><h2 id="patch-highlights">Destaques da Atualização</h2></header>
          <img src="destaques.jpg">
          <header class="header-primary"><h2>Campeões</h2></header>
          <div class="patch-change-block">
            <h3 class="change-title"><a href="/corki">Corki</a></h3>
            <blockquote class="blockquote context"><p>Corki está forte&nbsp;demais.</p></blockquote>
            <hr class="divider">
            <h4 class="change-detail-title ability-title"><img src="q.png">Q - Bomba de Fósforo</h4>
            <ul><li><strong>Dano</strong>: 90/170/250 <span>⇒</span> 80/160/240</li>
              <li>Recarga<br>menor<ul><li>Só no nível 1</li></ul></li></ul>
          </div>
          <header class="header-primary"><h2>Correções de bugs</h2></header>
          <ul><li>Mel não trava mais.</li><li></li></ul>
          <script>var x = "<h2>Falso</h2><p>falso</p>";</script>
          <header><h2>Artigos relacionados</h2></header>
          <h3>Outro artigo</h3>
        </div></body></html>"#;

    fn line(kind: PatchLineKind, text: &str) -> PatchLine {
        PatchLine {
            kind,
            text: text.into(),
        }
    }

    #[test]
    fn le_as_secoes_do_corpo_da_pagina() {
        use PatchLineKind::*;
        assert_eq!(
            parse_sections(NOTES_PAGE),
            vec![
                PatchSection {
                    title: "Campeões".into(),
                    changes: vec![PatchChange {
                        title: Some("Corki".into()),
                        lines: vec![
                            line(Text, "Corki está forte demais."),
                            line(Heading, "Q - Bomba de Fósforo"),
                            line(Bullet, "Dano: 90/170/250 ⇒ 80/160/240"),
                            line(Bullet, "Recarga menor"),
                            line(Bullet, "Só no nível 1"),
                        ],
                    }],
                },
                PatchSection {
                    title: "Correções de bugs".into(),
                    changes: vec![PatchChange {
                        title: None,
                        lines: vec![line(Bullet, "Mel não trava mais.")],
                    }],
                },
            ]
        );
    }

    #[test]
    fn pagina_sem_corpo_nao_tem_secoes() {
        assert!(parse_sections("").is_empty());
        assert!(parse_sections("<h1>Notas</h1><p>sem seção</p><h2").is_empty());
        assert!(parse_sections("<h2>Só título</h2><script>").is_empty());
    }

    #[test]
    fn texto_longo_e_cortado() {
        let html = format!("<h2>A</h2><p>{}</p>", "x".repeat(MAX_TEXT_CHARS + 10));
        let sections = parse_sections(&html);
        let text = &sections[0].changes[0].lines[0].text;
        assert_eq!(text.chars().count(), MAX_TEXT_CHARS);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn so_abre_o_site_oficial() {
        assert!(allowed_url(
            "https://www.leagueoflegends.com/pt-br/news/tags/patch-notes/"
        ));
        assert!(!allowed_url("https://example.com/"));
        assert!(!allowed_url("https://www.leagueoflegends.com.evil.com/"));
        assert!(!allowed_url("https://www.leagueoflegends.com/a b"));
        assert!(!allowed_url("file:///C:/Windows"));
    }
}
