//! Tipos TypeScript do protocolo do `/ws`, gerados com `ts-rs` para
//! `ui/src/bindings/`.
//!
//! Os arquivos saem de [`ServerMessage`] e [`ClientMessage`], seguindo as
//! dependências: um tipo novo usado no protocolo entra sozinho, sem lista
//! para atualizar. Depois de mudar qualquer tipo do protocolo, rode
//! `EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings` e
//! comite os arquivos gerados. O teste `bindings_estao_atualizados` falha no
//! CI se alguém esquecer.
//!
//! O `ts-rs` escreve cada tipo numa linha só, e aí dois PRs que acrescentam
//! campos ou mensagens diferentes sempre conflitam. Por isso [`pretty`]
//! quebra a declaração: uma variante da união por linha e um campo do
//! objeto por linha.

use std::any::TypeId;
use std::collections::HashSet;

use ts_rs::{Config, ExportError, TypeVisitor, TS};

use crate::assets::AssetKind;

use super::protocol::{ClientMessage, ServerMessage};

/// Gera os arquivos `.ts` de todos os tipos do protocolo: `(nome do arquivo,
/// conteúdo)`, em ordem de nome.
pub fn typescript_bindings() -> Result<Vec<(String, String)>, ExportError> {
    let cfg = Config::default();
    let mut collector = Collector {
        cfg: &cfg,
        seen: HashSet::new(),
        files: Vec::new(),
        error: None,
    };
    collector.visit::<ServerMessage>();
    collector.visit::<ClientMessage>();
    // Usado pela rota de imagens, não pelas mensagens.
    collector.visit::<AssetKind>();
    if let Some(error) = collector.error {
        return Err(error);
    }
    let mut files = collector.files;
    files.sort();
    Ok(files)
}

/// Junta o arquivo de cada tipo e, recursivamente, dos tipos que ele usa.
struct Collector<'a> {
    cfg: &'a Config,
    seen: HashSet<TypeId>,
    files: Vec<(String, String)>,
    error: Option<ExportError>,
}

impl TypeVisitor for Collector<'_> {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        if self.error.is_some() || !self.seen.insert(TypeId::of::<T>()) {
            return;
        }
        // Tipos sem arquivo próprio (`String`, `Vec`...) não geram nada,
        // mas os parâmetros deles podem gerar.
        if let Some(path) = T::output_path() {
            let name = path.to_string_lossy().replace('\\', "/");
            match T::export_to_string(self.cfg) {
                Ok(content) => self.files.push((name, pretty(&content))),
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        T::visit_dependencies(self);
        T::visit_generics(self);
    }
}

/// Quebra a declaração `export type X = ...;` de um arquivo do `ts-rs`: numa
/// união, uma variante por linha; num objeto, um campo por linha. Objetos
/// dentro de uma variante ou de um campo ficam numa linha, a não ser que
/// tenham comentários. O resto do arquivo (imports e documentação do tipo)
/// fica como está.
pub fn pretty(file: &str) -> String {
    const DECL: &str = "export type ";
    let Some(start) = file.find(DECL) else {
        return file.to_owned();
    };
    let Some(eq) = file[start..].find(" = ").map(|i| start + i) else {
        return file.to_owned();
    };
    let rhs = file[eq + 3..].trim_end();
    let Some(rhs) = rhs.strip_suffix(';') else {
        return file.to_owned();
    };
    let nodes = parse(&mut tokenize(rhs).into_iter().peekable());

    let mut out = String::from(&file[..eq]);
    if nodes.iter().any(|n| matches!(n, Node::Tok(Tok::Pipe))) {
        out.push_str(" =");
        for member in split(&nodes, Tok::Pipe) {
            out.push_str("\n  | ");
            render_inline(&mut out, member, 1);
        }
    } else {
        out.push_str(" = ");
        match nodes.as_slice() {
            [group @ Node::Group('{', _)] => render_block(&mut out, group, 0),
            _ => render_inline(&mut out, &nodes, 0),
        }
    }
    out.push_str(";\n");
    out
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// Comentário `/** ... */`, uma linha por item, sem o recuo.
    Comment(Vec<String>),
    Open(char),
    Close(char),
    Comma,
    Pipe,
    /// Qualquer outro texto: nomes, `campo:`, strings, `null`, `&`...
    Atom(String),
}

fn tokenize(text: &str) -> Vec<Tok> {
    let mut toks = Vec::new();
    let mut chars = text.char_indices().peekable();
    let mut atom = String::new();
    let flush = |atom: &mut String, toks: &mut Vec<Tok>| {
        if !atom.is_empty() {
            toks.push(Tok::Atom(std::mem::take(atom)));
        }
    };
    while let Some((i, c)) = chars.next() {
        match c {
            '/' if text[i..].starts_with("/*") => {
                flush(&mut atom, &mut toks);
                let end = text[i..].find("*/").map_or(text.len(), |e| i + e + 2);
                let lines = text[i..end].lines().map(|l| l.trim().to_owned()).collect();
                toks.push(Tok::Comment(lines));
                while chars.peek().is_some_and(|(j, _)| *j < end) {
                    chars.next();
                }
            }
            '"' => {
                atom.push(c);
                while let Some((_, c)) = chars.next() {
                    atom.push(c);
                    if c == '\\' {
                        if let Some((_, escaped)) = chars.next() {
                            atom.push(escaped);
                        }
                    } else if c == '"' {
                        break;
                    }
                }
            }
            '{' | '<' | '(' | '[' => {
                flush(&mut atom, &mut toks);
                toks.push(Tok::Open(c));
            }
            '}' | '>' | ')' | ']' => {
                flush(&mut atom, &mut toks);
                toks.push(Tok::Close(c));
            }
            ',' => {
                flush(&mut atom, &mut toks);
                toks.push(Tok::Comma);
            }
            '|' => {
                flush(&mut atom, &mut toks);
                toks.push(Tok::Pipe);
            }
            c if c.is_whitespace() => flush(&mut atom, &mut toks),
            c => atom.push(c),
        }
    }
    flush(&mut atom, &mut toks);
    toks
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Tok(Tok),
    /// Trecho entre `{}`, `<>`, `()` ou `[]`, sem os delimitadores.
    Group(char, Vec<Node>),
}

fn parse(toks: &mut std::iter::Peekable<std::vec::IntoIter<Tok>>) -> Vec<Node> {
    let mut nodes = Vec::new();
    while let Some(tok) = toks.next() {
        match tok {
            Tok::Open(c) => nodes.push(Node::Group(c, parse(toks))),
            Tok::Close(_) => break,
            tok => nodes.push(Node::Tok(tok)),
        }
    }
    nodes
}

/// Divide `nodes` em `sep`, no nível de cima. Pedaços vazios (como o que
/// sobra depois da vírgula final de um objeto) ficam de fora.
fn split(nodes: &[Node], sep: Tok) -> Vec<&[Node]> {
    nodes
        .split(|n| *n == Node::Tok(sep.clone()))
        .filter(|part| !part.is_empty())
        .collect()
}

fn has_comment(nodes: &[Node]) -> bool {
    nodes
        .iter()
        .any(|n| matches!(n, Node::Tok(Tok::Comment(_))))
}

fn indent(out: &mut String, level: usize) {
    out.push_str(&"  ".repeat(level));
}

/// Objeto com um campo por linha, cada um depois dos próprios comentários.
fn render_block(out: &mut String, group: &Node, level: usize) {
    let Node::Group(_, children) = group else {
        return;
    };
    out.push('{');
    for field in split(children, Tok::Comma) {
        let mut rest = field;
        while let [Node::Tok(Tok::Comment(lines)), tail @ ..] = rest {
            for line in lines {
                out.push('\n');
                indent(out, level + 1);
                if line.starts_with('*') {
                    out.push(' ');
                }
                out.push_str(line);
            }
            rest = tail;
        }
        out.push('\n');
        indent(out, level + 1);
        render_inline(out, rest, level + 1);
        out.push(',');
    }
    out.push('\n');
    indent(out, level);
    out.push('}');
}

/// Escreve `nodes` numa linha, no espaçamento do `ts-rs`. Um objeto com
/// comentários vira bloco.
fn render_inline(out: &mut String, nodes: &[Node], level: usize) {
    for node in nodes {
        match node {
            Node::Group('{', children) if has_comment(children) => {
                space(out);
                render_block(out, node, level);
            }
            Node::Group(open, children) => {
                if *open == '{' {
                    space(out);
                    out.push_str("{ ");
                } else {
                    out.push(*open);
                }
                render_inline(out, children, level);
                let close = match open {
                    '{' => '}',
                    '<' => '>',
                    '(' => ')',
                    _ => ']',
                };
                if close == '}' {
                    space(out);
                } else if out.ends_with(' ') {
                    out.pop();
                }
                out.push(close);
            }
            Node::Tok(Tok::Atom(atom)) => {
                space(out);
                out.push_str(atom);
            }
            Node::Tok(Tok::Comma) => out.push_str(", "),
            Node::Tok(Tok::Pipe) => {
                space(out);
                out.push_str("| ");
            }
            // Só chega aqui comentário fora de objeto, o que o `ts-rs` não
            // gera; vira uma linha só.
            Node::Tok(Tok::Comment(lines)) => {
                space(out);
                out.push_str(&lines.join(" "));
            }
            Node::Tok(Tok::Open(_) | Tok::Close(_)) => {}
        }
    }
}

/// Um espaço antes do próximo pedaço, a não ser no começo ou logo depois de
/// um espaço, de uma quebra ou de um `<`, `(` ou `[`.
fn space(out: &mut String) {
    if !matches!(
        out.chars().last(),
        None | Some(' ' | '\n' | '<' | '(' | '[')
    ) {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniao_fica_uma_variante_por_linha() {
        let file = "// nota\nimport type { Q } from \"./Q\";\n\nexport type C = { \"type\": \"ping\" } | { \"type\": \"createLobby\", queue: Q, } | { \"type\": \"x\", ids: Array<number>, map: Record<string, Q | null>, };\n";
        assert_eq!(
            pretty(file),
            "// nota\nimport type { Q } from \"./Q\";\n\nexport type C =\n  | { \"type\": \"ping\" }\n  | { \"type\": \"createLobby\", queue: Q, }\n  | { \"type\": \"x\", ids: Array<number>, map: Record<string, Q | null>, };\n"
        );
    }

    #[test]
    fn objeto_fica_um_campo_por_linha_com_os_comentarios() {
        let file = "/**\n * Doc.\n */\nexport type S = { a: A | null, b: string, \n/**\n * Campo b.\n * Segunda linha.\n */\nc: Array<C>, };\n";
        assert_eq!(
            pretty(file),
            "/**\n * Doc.\n */\nexport type S = {\n  a: A | null,\n  b: string,\n  /**\n   * Campo b.\n   * Segunda linha.\n   */\n  c: Array<C>,\n};\n"
        );
    }

    #[test]
    fn variante_com_comentario_vira_bloco() {
        let file =
            "export type M = { \"type\": \"hello\", \n/**\n * Versão.\n */\nprotocol: number, } | { \"type\": \"lcu\", status: L, };\n";
        assert_eq!(
            pretty(file),
            "export type M =\n  | {\n    \"type\": \"hello\",\n    /**\n     * Versão.\n     */\n    protocol: number,\n  }\n  | { \"type\": \"lcu\", status: L, };\n"
        );
    }

    #[test]
    fn uniao_de_strings_e_tipos_simples() {
        assert_eq!(
            pretty("export type P = \"top\" | \"jungle\";\n"),
            "export type P =\n  | \"top\"\n  | \"jungle\";\n"
        );
        assert_eq!(
            pretty("export type N = number;\n"),
            "export type N = number;\n"
        );
    }

    /// Compara os tipos gerados com os arquivos em `ui/src/bindings/`. Com
    /// `EASYLOBBY_UPDATE_BINDINGS=1`, regrava os arquivos em vez de comparar.
    #[test]
    fn bindings_estao_atualizados() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/src/bindings");
        let files = typescript_bindings().unwrap();
        if std::env::var_os("EASYLOBBY_UPDATE_BINDINGS").is_some() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().is_some_and(|e| e == "ts") {
                    std::fs::remove_file(path).unwrap();
                }
            }
            for (name, content) in &files {
                std::fs::write(dir.join(name), content).unwrap();
            }
            return;
        }

        let hint = "rode `EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings` \
                    e comite ui/src/bindings/ (num conflito nesses arquivos, resolva o Rust \
                    e rode o comando de novo)";
        let mut committed: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".ts"))
            .collect();
        committed.sort();
        let expected: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(
            committed, expected,
            "arquivos diferentes em ui/src/bindings: {hint}"
        );
        for (name, content) in &files {
            // O checkout no Windows pode trocar \n por \r\n.
            let on_disk = std::fs::read_to_string(dir.join(name))
                .unwrap()
                .replace("\r\n", "\n");
            assert_eq!(&on_disk, content, "{name} está desatualizado: {hint}");
        }
    }
}
