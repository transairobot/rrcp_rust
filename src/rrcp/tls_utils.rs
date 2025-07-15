use std::sync::Arc;

use rustls::crypto::{aws_lc_rs, CryptoProvider};
use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig, RootCertStore};
use rustls::{
    DigitallySignedStruct,
    client::danger::HandshakeSignatureValid,
    crypto::{verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};

use rustls::crypto::aws_lc_rs as provider;

#[derive(Debug)]
pub struct NoCertificateVerification(CryptoProvider);

impl NoCertificateVerification {
    pub fn new(provider: CryptoProvider) -> Self {
        Self(provider)
    }
}

impl rustls::client::danger::ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

pub(super) fn new_tls_client_config() -> anyhow::Result<ClientConfig> {
    let mut roots = RootCertStore::empty();
    for entry in std::fs::read_dir("/etc/ssl/certs")? {
        let entry = entry?;
        let path = entry.path();
        if path.to_str().unwrap().ends_with("pem") {
            roots.add(CertificateDer::from_pem_file(&path)?)?;
        }
    }
    Ok(rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth())
}

pub(super) fn new_danger_tls_client_config() -> anyhow::Result<ClientConfig> {
    let client_crypto = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(
            super::tls_utils::NoCertificateVerification::new(provider::default_provider()),
        ))
        .with_no_client_auth();
    return Ok(client_crypto);
}
