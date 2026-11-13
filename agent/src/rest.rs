//! Cliente REST da LCU API (#8).
//!
//! A LCU responde em `https://127.0.0.1:<porta>` com Basic Auth
//! `riot:<senha do lockfile>`. O certificado do servidor é assinado pela raiz
//! da Riot (`certs/riotgames.pem`), que o cliente fixa como **única** raiz
//! confiável: as raízes do sistema ficam de fora e a verificação TLS nunca é
//! desligada.
//!
//! O `riotgames.pem` é um certificado X.509 v1, que o `webpki` do rustls
//! recusa. Por isso o TLS é o nativo do sistema (`native-tls`: SChannel no
//! Windows, OpenSSL no Linux, Security.framework no macOS).
//!
//! Para falar com o mock-lcu, que gera uma CA própria a cada execução, use
//! [`LcuClient::with_root_certificate`] com o PEM dessa CA. A regra é a
//! mesma: um certificado fixado, sem desligar nada.
//!
//! ```no_run
//! # async fn exemplo() -> Result<(), Box<dyn std::error::Error>> {
//! use easylobby_agent::{lockfile, rest::LcuClient};
//!
//! let credentials = lockfile::discover()?;
//! let lcu = LcuClient::new(&credentials)?;
//! let summoner: serde_json::Value = lcu.get("/lol-summoner/v1/current-summoner").await?;
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::time::Duration;

use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::lockfile::{Credentials, LCU_USERNAME};

/// Certificado raiz da Riot que assina o HTTPS da LCU.
pub const RIOT_ROOT_PEM: &[u8] = include_bytes!("../certs/riotgames.pem");

/// Tempo máximo de uma requisição, do connect ao fim do corpo.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Resposta da LCU, com qualquer status.
#[derive(Debug, Clone, PartialEq)]
pub struct LcuResponse {
    pub status: u16,
    /// Corpo em JSON. `Null` quando vazio (por exemplo num `204`). Um corpo
    /// que não é JSON vira `Value::String`.
    pub body: Value,
}

impl LcuResponse {
    /// Se o status é `2xx`.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Erro ao montar o cliente ou ao falar com a LCU.
#[derive(Debug, Error)]
pub enum LcuError {
    #[error("certificado raiz inválido: {0}")]
    Certificate(#[source] reqwest::Error),
    #[error("não foi possível montar o cliente HTTP: {0}")]
    Build(#[source] reqwest::Error),
    #[error("{method} {path} falhou: {source}")]
    Request {
        method: Method,
        path: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{method} {path} respondeu {status}{}", message.as_ref().map(|m| format!(": {m}")).unwrap_or_default())]
    Status {
        method: Method,
        path: String,
        status: u16,
        /// Campo `message` do erro da LCU, quando houver.
        message: Option<String>,
        body: Value,
    },
    #[error("{method} {path}: não foi possível serializar o corpo: {source}")]
    Encode {
        method: Method,
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{method} {path}: resposta em formato inesperado: {source}")]
    Decode {
        method: Method,
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

impl LcuError {
    /// Status HTTP, quando a LCU chegou a responder.
    pub fn status(&self) -> Option<u16> {
        match self {
            LcuError::Status { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Se o erro foi de TLS ou de conexão (LoL fechado, porta errada,
    /// certificado que não bate com o fixado).
    pub fn is_connect(&self) -> bool {
        matches!(self, LcuError::Request { source, .. } if source.is_connect())
    }
}

/// Cliente REST da LCU, com Basic Auth e o certificado fixado.
///
/// Barato de clonar: o pool de conexões é compartilhado. O `Debug` esconde a
/// senha.
#[derive(Clone)]
pub struct LcuClient {
    http: reqwest::Client,
    base_url: String,
    password: String,
}

impl LcuClient {
    /// Cliente para o LoL de verdade, com o `riotgames.pem` fixado.
    pub fn new(credentials: &Credentials) -> Result<Self, LcuError> {
        Self::with_root_certificate(credentials, RIOT_ROOT_PEM)
    }

    /// Cliente com outra raiz fixada no lugar da Riot, como a CA do mock-lcu.
    ///
    /// `root_pem` é a única raiz aceita; as do sistema não entram.
    pub fn with_root_certificate(
        credentials: &Credentials,
        root_pem: &[u8],
    ) -> Result<Self, LcuError> {
        Self::build(credentials, root_pem, DEFAULT_TIMEOUT)
    }

    /// Como [`LcuClient::with_root_certificate`], com outro tempo máximo por
    /// requisição.
    pub fn build(
        credentials: &Credentials,
        root_pem: &[u8],
        timeout: Duration,
    ) -> Result<Self, LcuError> {
        let root = reqwest::Certificate::from_pem(root_pem).map_err(LcuError::Certificate)?;
        let http = reqwest::Client::builder()
            .use_native_tls()
            .tls_built_in_root_certs(false)
            .add_root_certificate(root)
            // A LCU só escuta em 127.0.0.1; proxy do sistema não tem o que
            // fazer aqui e veria a senha.
            .no_proxy()
            .timeout(timeout)
            .build()
            .map_err(LcuError::Build)?;
        Ok(Self {
            http,
            base_url: credentials.base_url(),
            password: credentials.password.clone(),
        })
    }

    /// URL base, por exemplo `https://127.0.0.1:54321`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Faz a requisição e devolve status e corpo, sem tratar status de erro.
    ///
    /// `path` começa com `/` e pode ter query (`/foo?bar=1`).
    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<LcuResponse, LcuError> {
        let request_err = |source| LcuError::Request {
            method: method.clone(),
            path: path.to_owned(),
            source,
        };

        let mut request = self
            .http
            .request(method.clone(), format!("{}{path}", self.base_url))
            .basic_auth(LCU_USERNAME, Some(&self.password))
            .header(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(body) = body {
            request = request
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(body.to_string());
        }

        let response = request.send().await.map_err(request_err)?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(request_err)?;
        Ok(LcuResponse {
            status,
            body: parse_body(&text),
        })
    }

    /// Faz a requisição e converte o corpo de uma resposta `2xx` em `T`.
    ///
    /// Status fora de `2xx` vira [`LcuError::Status`]. Para respostas sem
    /// corpo, use `T = ()` ou `Option<_>`.
    pub async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<T, LcuError> {
        let response = self.request(method.clone(), path, body).await?;
        if !response.is_success() {
            return Err(LcuError::Status {
                method,
                path: path.to_owned(),
                status: response.status,
                message: error_message(&response.body),
                body: response.body,
            });
        }
        serde_json::from_value(response.body).map_err(|source| LcuError::Decode {
            method,
            path: path.to_owned(),
            source,
        })
    }

    /// `GET` de um arquivo da LCU, como uma imagem de `/lol-game-data/assets/`.
    /// `None` quando a LCU responde fora de `2xx` ou o arquivo passa de
    /// `max_bytes`.
    pub async fn get_file(
        &self,
        path: &str,
        max_bytes: usize,
    ) -> Result<Option<Vec<u8>>, LcuError> {
        let request_err = |source| LcuError::Request {
            method: Method::GET,
            path: path.to_owned(),
            source,
        };
        let mut response = self
            .http
            .get(format!("{}{path}", self.base_url))
            .basic_auth(LCU_USERNAME, Some(&self.password))
            .send()
            .await
            .map_err(request_err)?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(request_err)? {
            if bytes.len() + chunk.len() > max_bytes {
                return Ok(None);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Some(bytes))
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, LcuError> {
        self.send(Method::GET, path, None).await
    }

    pub async fn post<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, LcuError> {
        self.send_with(Method::POST, path, body).await
    }

    pub async fn put<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, LcuError> {
        self.send_with(Method::PUT, path, body).await
    }

    pub async fn patch<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, LcuError> {
        self.send_with(Method::PATCH, path, body).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T, LcuError> {
        self.send(Method::DELETE, path, None).await
    }

    async fn send_with<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
    ) -> Result<T, LcuError> {
        let body = serde_json::to_value(body).map_err(|source| LcuError::Encode {
            method: method.clone(),
            path: path.to_owned(),
            source,
        })?;
        self.send(method, path, Some(&body)).await
    }
}

impl fmt::Debug for LcuClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcuClient")
            .field("base_url", &self.base_url)
            .field("password", &"<oculta>")
            .finish()
    }
}

/// Corpo em JSON; vazio vira `Null` e texto que não é JSON vira `String`.
fn parse_body(text: &str) -> Value {
    if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
    }
}

/// `message` do erro da LCU (`{"errorCode": ..., "httpStatus": ..., "message": ...}`).
fn error_message(body: &Value) -> Option<String> {
    match body {
        Value::Object(map) => map
            .get("message")
            .and_then(Value::as_str)
            .filter(|m| !m.is_empty())
            .map(str::to_owned),
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::{Protocol, Source};
    use serde_json::json;

    const SENHA_FALSA: &str = "senha-falsa-de-teste";

    fn credentials() -> Credentials {
        Credentials {
            pid: Some(1),
            port: 54321,
            password: SENHA_FALSA.into(),
            protocol: Protocol::Https,
            source: Source::ProcessArgs { pid: 1 },
        }
    }

    #[test]
    fn monta_com_o_certificado_da_riot() {
        let lcu = LcuClient::new(&credentials()).unwrap();
        assert_eq!(lcu.base_url(), "https://127.0.0.1:54321");
    }

    #[test]
    fn recusa_certificado_invalido() {
        let err = LcuClient::with_root_certificate(&credentials(), b"nao e um pem").unwrap_err();
        assert!(matches!(err, LcuError::Certificate(_)), "{err}");
    }

    #[test]
    fn debug_nao_mostra_a_senha() {
        let debug = format!("{:?}", LcuClient::new(&credentials()).unwrap());
        assert!(!debug.contains(SENHA_FALSA), "{debug}");
        assert!(debug.contains("54321"));
    }

    #[test]
    fn interpreta_o_corpo() {
        assert_eq!(parse_body(""), Value::Null);
        assert_eq!(parse_body(" \n"), Value::Null);
        assert_eq!(parse_body("\"Lobby\""), json!("Lobby"));
        assert_eq!(parse_body("{\"a\":1}"), json!({"a": 1}));
        assert_eq!(parse_body("nao e json"), json!("nao e json"));
    }

    #[test]
    fn tira_a_mensagem_do_erro_da_lcu() {
        let body = json!({
            "errorCode": "RPC_ERROR",
            "httpStatus": 404,
            "message": "No active delegate",
        });
        assert_eq!(error_message(&body).as_deref(), Some("No active delegate"));
        assert_eq!(error_message(&json!({"message": ""})), None);
        assert_eq!(error_message(&json!("texto")).as_deref(), Some("texto"));
        assert_eq!(error_message(&Value::Null), None);
    }

    #[test]
    fn mensagem_do_erro_de_status() {
        let err = LcuError::Status {
            method: Method::GET,
            path: "/lol-lobby/v2/lobby".into(),
            status: 404,
            message: Some("No active delegate".into()),
            body: Value::Null,
        };
        assert_eq!(
            err.to_string(),
            "GET /lol-lobby/v2/lobby respondeu 404: No active delegate"
        );
        assert_eq!(err.status(), Some(404));
    }
}
