// use rustls::Certificate;
use std::fs::{self, File};
use std::io::{BufReader, Read};

use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;

pub fn load_certificates_from_pem(path: &str) -> anyhow::Result<RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(fs::read(path)?));
    roots.
    Ok(roots)
}

// pub fn load_private_key(path: &str) -> rustls::PrivateKey {
//     let key = fs::read(path).expect(format!("read {} failed.", path).as_str());
//     let key = rustls_pemfile::private_key(&mut &*key)
//         .expect("malformed PKCS #1 private key")
//         .unwrap()
//         .secret_der()
//         .to_vec();
//     return rustls::PrivateKey(key);
// }
