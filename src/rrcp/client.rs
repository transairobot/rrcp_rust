use crate::rrcp::tls_utils;

use super::stream_pool::StreamPool;
use log::debug;
use log::info;
use quinn::{Endpoint, RecvStream, crypto::rustls::QuicClientConfig};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tokio::io::AsyncWriteExt;

use super::header::*;
use super::message_pack_proto::*;

pub struct RrcpClient {
    server_addr: SocketAddr,
    endpoint: Endpoint,
    stream_pool: StreamPool,
}

#[derive(Debug, PartialEq)]
pub struct RrcpFrame {
    header: RRCPHeader,
    body: Vec<u8>,
}

impl RrcpFrame {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = self.header.to_bytes();
        buf.extend_from_slice(&self.body);
        buf
    }
}

impl RrcpClient {
    async fn do_request(&mut self, req: &[u8]) -> anyhow::Result<RrcpFrame> {
        let (mut send_stream, mut recv_stream) = self.stream_pool.get_bi_stream().await?;
        send_stream.write_all(req).await?;
        send_stream.flush().await?;
        let frame = Self::read_rrcp_frame(&mut recv_stream).await?;
        Ok(frame)
    }

    async fn read_rrcp_frame(recv_stream: &mut RecvStream) -> anyhow::Result<RrcpFrame> {
        let mut buf = vec![0; std::mem::size_of::<RRCPHeader>()];
        recv_stream.read_exact(&mut buf).await?;
        let header = RRCPHeader::from_bytes(&buf)?;

        let mut body = vec![0; header.body_length as usize];
        recv_stream.read_exact(&mut body).await?;

        Ok(RrcpFrame { header, body })
    }

    pub async fn get_config(&mut self) -> anyhow::Result<RrcpConfig> {
        let header = RRCPHeader::new_with_flag(Flag::GetConfig).to_bytes();
        let frame = self.do_request(&header).await?;

        // info!("Received frame: {:?}", frame.header);
        // info!("body={:?}", frame.body);
        let config =
            RrcpConfig::deserialize(&mut Deserializer::from_read_ref(&frame.body)).unwrap();

        // info!("Received config: {:?}", config);
        Ok(config)
    }

    pub async fn get_action(&mut self, sensor_data: &SensorData) -> anyhow::Result<Action> {
        let mut header = RRCPHeader::new_with_flag(Flag::GetAction);

        let mut body = Vec::new();
        sensor_data
            .serialize(&mut Serializer::new(&mut body).with_struct_map())
            .unwrap();
        println!();
        println!();
        for ch in &body {
            print!("{:x} ", ch);
        }
        println!();
        println!();
        header.body_length = body.len() as u64;
        let req_frame = RrcpFrame {
            header,
            body,
        };

        info!("Sending request: {:?}", &req_frame);
        let frame = self.do_request(&req_frame.to_bytes()).await?;

        let action = Action::deserialize(&mut Deserializer::from_read_ref(&frame.body)).unwrap();
        info!("Received frame: {:?}", frame.header);
        // info!("body={:?}", frame.body);
        // 处理业务逻辑
        Ok(action)
    }

    pub async fn new(server_addr: SocketAddr, server_name: &str) -> anyhow::Result<Self> {
        debug!("new robot client");
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("install provider failed");

        let mut client_crypto = tls_utils::new_tls_client_config()?;
        client_crypto.alpn_protocols = vec![b"quic".as_ref().into()];
        let client_config =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto)?));

        let mut endpoint = Endpoint::client("[::]:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        // // 2. 连接服务器
        let connection = endpoint.connect(server_addr, server_name)?.await?;
        info!("已连接到服务器: {}", connection.remote_address());

        Ok(Self {
            server_addr,
            endpoint,
            stream_pool: StreamPool::new(10, connection).await?,
        })
    }
}
