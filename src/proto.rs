use crate::stream_pool::StreamPool;
use crate::utils;
use log::{debug, error, info};
use quinn::{Connection, Endpoint, RecvStream, SendStream, crypto::rustls::QuicClientConfig};
use rmp_serde::{Deserializer, Serializer};
use rustls::crypto::{CryptoProvider, SupportedKxGroup, aws_lc_rs as provider};
use rustls::{
    DigitallySignedStruct,
    client::danger::HandshakeSignatureValid,
    crypto::{verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use serde::{Deserialize, Serialize};
use std::any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{net::SocketAddr, sync::Arc};
use tokio::io::AsyncWriteExt;
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

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentType {
    None = 0,
    MessagePack = 1,
}

impl ContentType {
    fn from_u16(value: u16) -> anyhow::Result<Self> {
        match value {
            0 => Ok(ContentType::None),
            1 => Ok(ContentType::MessagePack),
            _ => Err(anyhow::anyhow!("Unknown content type: {}", value)),
        }
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    None = 0,
    GetConfig = 1,
    GetAction = 2,
}
impl Flag {
    fn from_u16(value: u16) -> anyhow::Result<Self> {
        match value {
            1 => Ok(Flag::GetConfig),
            2 => Ok(Flag::GetAction),
            _ => anyhow::bail!("Unknown flag value: {}", value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
#[repr(packed)]
struct RRCPHeader {
    magic: u32,
    version: u32,
    body_length: u64,
    server_timestamp_ms: u64,
    content_type: ContentType,
    flag: Flag,
}

pub fn now_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

const MAGIC_NUMBER: u32 = 0x7312;
impl RRCPHeader {
    pub fn new_with_flag(flag: Flag) -> Self {
        Self {
            magic: MAGIC_NUMBER,
            version: 1,
            body_length: 0,
            server_timestamp_ms: now_timestamp_ms(),
            content_type: ContentType::MessagePack,
            flag,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(std::mem::size_of::<Self>());
        buf.extend_from_slice(&self.magic.to_le_bytes());
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.body_length.to_le_bytes());
        buf.extend_from_slice(&self.server_timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&(self.content_type as u16).to_le_bytes());
        buf.extend_from_slice(&(self.flag as u16).to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() < std::mem::size_of::<Self>() {
            return Err(anyhow::anyhow!("Bytes too short for RRCPHeader"));
        }
        if bytes.len() < std::mem::size_of::<Self>() {
            return Err(anyhow::anyhow!("Bytes too short for RRCPHeader"));
        }
        let mut magic = [0; 4];
        let mut version = [0; 4];
        let mut body_length = [0; 8];
        let mut server_timestamp_ms = [0; 8];
        let mut content_type = [0; 2];
        let mut flag = [0; 2];
        let mut seq = [0; 8];

        magic.copy_from_slice(&bytes[0..4]);
        version.copy_from_slice(&bytes[4..8]);
        body_length.copy_from_slice(&bytes[8..16]);
        server_timestamp_ms.copy_from_slice(&bytes[16..24]);
        content_type.copy_from_slice(&bytes[24..26]);
        flag.copy_from_slice(&bytes[26..28]);
        seq.copy_from_slice(&bytes[28..36]);

        let magic = u32::from_le_bytes(magic);
        if magic != MAGIC_NUMBER {
            return Err(anyhow::anyhow!("Invalid magic number: {}", magic));
        }
        Ok(Self {
            magic,
            version: u32::from_le_bytes(version),
            body_length: u64::from_le_bytes(body_length),
            server_timestamp_ms: u64::from_le_bytes(server_timestamp_ms),
            content_type: ContentType::from_u16(u16::from_le_bytes(content_type))?,
            flag: Flag::from_u16(u16::from_le_bytes(flag))?,
        })
    }
}

pub struct RrcpClient {
    server_addr: SocketAddr,
    endpoint: Endpoint,
    stream_pool: StreamPool,
    seq_id: AtomicU64,
}

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

        println!("Received frame: {:?}", frame.header);
        println!("body={:?}", frame.body);
        let config =
            RrcpConfig::deserialize(&mut Deserializer::from_read_ref(&frame.body)).unwrap();

        info!("Received config: {:?}", config);
        return Ok(config);
    }

    pub async fn get_action(&mut self, ob: &Ob) -> anyhow::Result<Action> {
        let header = RRCPHeader::new_with_flag(Flag::GetAction);
        let mut body = Vec::new();
        ob.serialize(&mut Serializer::new(&mut body).with_struct_map())
            .unwrap();

        let req_frame = RrcpFrame {
            header: header,
            body: body,
        };
        let frame = self.do_request(&req_frame.to_bytes()).await?;

        let action = Action::deserialize(&mut Deserializer::from_read_ref(&frame.body)).unwrap();
        println!("Received frame: {:?}", frame.header);
        println!("body={:?}", frame.body);
        // 处理业务逻辑
        Ok(action)
    }

    pub async fn new(server_addr: SocketAddr) -> anyhow::Result<Self> {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("install provider failed");
        // // 1. 配置客户端（不验证证书，仅用于测试）
        // let roots = utils::load_certificates_from_pem("cert.pem").unwrap();
        // let mut client_crypto = rustls::ClientConfig::builder()
        //     .with_root_certificates(roots)
        //     .with_no_client_auth();
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertificateVerification::new(
                provider::default_provider(),
            )))
            .with_no_client_auth();

        client_crypto.alpn_protocols = vec![b"quic".as_ref().into()];
        let client_config =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_crypto)?));

        let mut endpoint = Endpoint::client("[::]:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        // // 2. 连接服务器
        let connection = endpoint.connect(server_addr, "localhost")?.await?;
        info!("已连接到服务器: {}", connection.remote_address());

        Ok(Self {
            server_addr: server_addr,
            endpoint: endpoint,
            stream_pool: StreamPool::new(10, connection).await?,
            seq_id: AtomicU64::new(0),
        })
    }
}
