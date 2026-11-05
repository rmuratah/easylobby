//! Confere todo conjunto em `tests/fixtures/<patch>/` (#5): os arquivos abrem,
//! seguem o formato e não têm dados reais.

use std::fs;
use std::path::{Path, PathBuf};

use easylobby_agent::fixtures::{FixtureSet, Sanitizer, FORMAT_VERSION};
use serde_json::Value;

fn fixture_dirs() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut dirs: Vec<_> = fs::read_dir(&root)
        .expect("tests/fixtures existe")
        .map(|e| e.expect("entrada legível").path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn load_all() -> Vec<(PathBuf, FixtureSet)> {
    fixture_dirs()
        .into_iter()
        .map(|dir| {
            let set = FixtureSet::load(&dir).unwrap_or_else(|e| panic!("{e}"));
            (dir, set)
        })
        .collect()
}

#[test]
fn existe_pelo_menos_um_conjunto() {
    assert!(!fixture_dirs().is_empty());
}

#[test]
fn manifest_bate_com_a_pasta() {
    for (dir, set) in load_all() {
        let name = dir.file_name().unwrap().to_string_lossy();
        assert_eq!(set.manifest.patch, name, "{}", dir.display());
        assert_eq!(
            set.manifest.format_version,
            FORMAT_VERSION,
            "{}",
            dir.display()
        );
    }
}

#[test]
fn respostas_e_eventos_sao_validos() {
    for (dir, set) in load_all() {
        assert!(!set.rest.is_empty(), "{} sem respostas REST", dir.display());
        for r in &set.rest {
            assert_eq!(r.method, "GET", "{} {}", dir.display(), r.path);
            assert!(r.path.starts_with('/'), "{} {}", dir.display(), r.path);
            assert!(!r.phase.is_empty(), "{} {}", dir.display(), r.path);
        }
        for e in &set.events {
            assert!(e.uri.starts_with('/'), "{} {}", dir.display(), e.uri);
            assert!(
                ["Create", "Update", "Delete"].contains(&e.event_type.as_str()),
                "{} {} {}",
                dir.display(),
                e.uri,
                e.event_type
            );
        }
        let times: Vec<_> = set.events.iter().map(|e| e.t_ms).collect();
        assert!(
            times.windows(2).all(|w| w[0] <= w[1]),
            "{}: eventos fora de ordem",
            dir.display()
        );
    }
}

/// Se sanitizar de novo mudasse algo, sobrou dado real numa chave conhecida.
#[test]
fn ja_estao_sanitizadas() {
    for (dir, set) in load_all() {
        let mut s = Sanitizer::new();
        for r in &set.rest {
            assert_eq!(
                s.sanitize_resource(&r.path, &r.body),
                r.body,
                "{} {} tem dado real",
                dir.display(),
                r.path
            );
        }
        for e in &set.events {
            assert_eq!(
                s.sanitize_resource(&e.uri, &e.data),
                e.data,
                "{} {} tem dado real",
                dir.display(),
                e.uri
            );
        }
    }
}

/// Puuids reais têm 78 caracteres de base64url e tokens (JWT) são ainda mais
/// longos, sem ponto nem barra. Pega o que escapou do sanitizer por estar numa
/// chave que ele não conhece. Caminhos de assets, versões e UUIDs de lobby
/// não entram.
#[test]
fn nenhum_texto_parece_id_ou_token_real() {
    fn check(dir: &Path, value: &Value) {
        match value {
            Value::String(s) => {
                let looks_like_id = s.len() >= 60
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                assert!(
                    !looks_like_id,
                    "{}: texto parece id ou token real: {s}",
                    dir.display()
                );
            }
            Value::Array(items) => items.iter().for_each(|v| check(dir, v)),
            Value::Object(map) => {
                map.keys()
                    .for_each(|k| check(dir, &Value::String(k.clone())));
                map.values().for_each(|v| check(dir, v));
            }
            _ => {}
        }
    }
    for (dir, set) in load_all() {
        set.rest.iter().for_each(|r| check(&dir, &r.body));
        set.events.iter().for_each(|e| check(&dir, &e.data));
    }
}
