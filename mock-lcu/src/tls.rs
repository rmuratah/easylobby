//! Certificados do mock-lcu.
//!
//! A LCU real usa um certificado assinado pela raiz da Riot
//! (`agent/certs/riotgames.pem`), cuja chave privada só a Riot tem. O mock
//! gera a cada execução uma CA própria e um certificado para `127.0.0.1` e
//! `localhost` assinado por ela. O PEM da CA vai para um arquivo ao lado do
//! lockfile, e o cliente fixa esse PEM do mesmo jeito que fixa o da Riot: a
//! verificação TLS nunca é desligada.

use std::sync::Arc;

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Certificados gerados para uma execução.
pub struct Generated {
    /// PEM da CA, para o cliente fixar.
    pub ca_pem: String,
    /// Configuração do servidor TLS.
    pub server_config: Arc<rustls::ServerConfig>,
}

/// Gera a CA, o certificado do servidor e a configuração do rustls.
pub fn generate() -> Result<Generated, String> {
    let err = |e: rcgen::Error| e.to_string();

    let mut ca_params = CertificateParams::new(Vec::<String>::new()).map_err(err)?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "easylobby mock-lcu CA");
    ca_params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];
    let ca_key = KeyPair::generate().map_err(err)?;
    let ca_cert = ca_params.self_signed(&ca_key).map_err(err)?;

    let mut leaf_params =
        CertificateParams::new(vec!["127.0.0.1".to_owned(), "localhost".to_owned()])
            .map_err(err)?;
    leaf_params
        .distinguished_name
        .push(DnType::CommonName, "127.0.0.1");
    leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    let leaf_key = KeyPair::generate().map_err(err)?;
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &ca_cert, &ca_key)
        .map_err(err)?;

    let chain = vec![
        CertificateDer::from(leaf_cert.der().to_vec()),
        CertificateDer::from(ca_cert.der().to_vec()),
    ];
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .map_err(|e| e.to_string())?;
    // Como a LCU: só HTTP/1.1, que é o que o WebSocket precisa.
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    Ok(Generated {
        ca_pem: ca_cert.pem(),
        server_config: Arc::new(config),
    })
}
