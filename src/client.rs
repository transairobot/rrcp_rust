use log::{debug, error, info};
use quinn::{Endpoint, TransportConfig};
use std::{net::SocketAddr, sync::Arc};

use crate::utils;

pub async fn run_client(server_addr: SocketAddr) -> anyhow::Result<()> {
    // // 1. 配置客户端（不验证证书，仅用于测试）
    let mut roots = rustls::RootCertStore::empty();
    let certs = utils::load_certificates_from_pem("cert/ca(1).crt").unwrap();
    certs.into_iter().for_each(|x| roots.add(&x).unwrap());

    let mut client_crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(roots)
        .with_no_client_auth();
    
    let mut endpoint = Endpoint::client("[::]:0".parse()?)?;
    let client_config = quinn::ClientConfig::new(Arc::new(client_crypto));
    endpoint.set_default_client_config(client_config);

    // // 2. 连接服务器
    let connection = endpoint.connect(server_addr, "localhost")?.await?;
    info!("已连接到服务器: {}", connection.remote_address());

    // 3. 打开新流
    let (mut send, mut recv) = connection.open_bi().await?;

    // 4. 发送消息
    let message = "你好服务器，这是QUIC客户端!";
    send.write_all(message.as_bytes()).await?;
    send.finish().await?;

    // 5. 接收响应
    let mut buf = vec![0; 1024];
    let len = recv.read(&mut buf).await?.unwrap_or(0);
    let response = String::from_utf8_lossy(&buf[..len]);

    Ok(())
}
