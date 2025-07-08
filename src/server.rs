use anyhow::anyhow;
use log::{error, info};
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use tracing::info_span;
use tracing_futures::Instrument as _;

pub async fn run_server(addr: SocketAddr) -> anyhow::Result<()> {
    // 1. 生成自签名证书（生产环境应使用正式证书）
    let (cert, key) = generate_self_signed_cert()?;

    // 2. 配置服务器
    let mut server_config = ServerConfig::with_single_cert(cert, key)?;
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(2)));
    server_config.transport = Arc::new(transport_config);

    // 3. 创建端点
    let endpoint = Endpoint::server(server_config, addr)?;
    info!("QUIC 服务器监听在: {}", addr);

    // 4. 接受新连接
    while let Some(conn) = endpoint.accept().await {
        let connection = conn.await?;
        info!("新连接来自: {}", connection.remote_address());

        // 5. 为每个连接创建异步任务
        tokio::spawn(async move {
            if let Err(e) = handle_connection(connection).await {
                error!("连接处理失败: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(connection: quinn::Connection) -> anyhow::Result<()> {
    let span = info_span!(
        "connection",
        remote = %connection.remote_address(),
        protocol = %connection
            .handshake_data()
            .unwrap()
            .downcast::<quinn::crypto::rustls::HandshakeData>().unwrap()
            .protocol
            .map_or_else(|| "<none>".into(), |x| String::from_utf8_lossy(&x).into_owned())
    );
    async {
        info!("established");

        // Each stream initiated by the client constitutes a new request.
        loop {
            let stream = connection.accept_bi().await;
            let stream = match stream {
                Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
                    info!("connection closed");
                    return Ok(());
                }
                Err(e) => {
                    return Err(e);
                }
                Ok(s) => s,
            };
            let fut = handle_request(stream);
            tokio::spawn(
                async move {
                    if let Err(e) = fut.await {
                        error!("failed: {reason}", reason = e.to_string());
                    }
                }
                .instrument(info_span!("request")),
            );
        }
    }
    .instrument(span)
    .await?;
    Ok(())
}

async fn handle_request(
    (mut send, mut recv): (quinn::SendStream, quinn::RecvStream),
) -> anyhow::Result<()> {
    let req = recv
        .read_to_end(64 * 1024)
        .await
        .map_err(|e| anyhow!("failed reading request: {}", e))?;
    let str = String::from_utf8_lossy(&req);
    println!("str={}", str);
    info!("complete");
    Ok(())
}

use rcgen::generate_simple_self_signed;
use rustls::{Certificate, PrivateKey};
use std::sync::Arc;

fn generate_self_signed_cert() -> anyhow::Result<(Vec<Certificate>, PrivateKey)> {
    let cert = generate_simple_self_signed(vec!["localhost".into()])?;
    let x = cert.serialize_pem().unwrap();
    std::fs::write("cert/cert.pem", x.as_bytes()).unwrap();
    let key = PrivateKey(cert.serialize_private_key_der());
    std::fs::write("cert/private.pem", key.as_bytes()).unwrap();
    let cert = Certificate(cert.serialize_der()?);
    Ok((vec![cert], key))
}
