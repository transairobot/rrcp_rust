use log::info;
use quinn::{Endpoint, ServerConfig};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use simple_logger::SimpleLogger;
use std::{net::SocketAddr, time::Duration};
use tokio::time::sleep;

mod rrcp;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

use crate::rrcp::message_pack_proto::{Image, SensorData, ServoStatus};

fn generate_self_signed_cert() -> anyhow::Result<()> {
    let cert = generate_simple_self_signed(vec!["localhost".into()])?;
    std::fs::write("cert.pem", cert.serialize_pem().unwrap()).unwrap();
    std::fs::write("private.pem", cert.serialize_private_key_pem()).unwrap();
    Ok(())
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct Human {
    age: u32,
    name: String,
    // #[serde(with = "serde_bytes")]
    data: Vec<u8>,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct HumanTmp {
    age: u32,
    data: Vec<u8>,
    name: String,
    // #[serde(with = "serde_bytes")]
}

fn print_an_address() {
    let mut buf = Vec::new();
    let val = Human {
        age: 42,
        name: "John".into(),
        data: vec![1; 100],
    };

    val.serialize(&mut Serializer::new(&mut buf).with_struct_map())
        .unwrap();

    let human = HumanTmp::deserialize(&mut Deserializer::from_read_ref(&buf)).unwrap();
    // Print, write to a file, or send to an HTTP server.
    println!("{}", buf.len());
    println!("{:?}", human);
}

#[tokio::main(worker_threads = 10)]
async fn main() -> anyhow::Result<()> {
    // 初始化 simple_logger
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();
    // generate_self_signed_cert()?;
    let server_addr: SocketAddr = "172.20.2.17:8080".parse()?;

    // // 在单独任务中运行服务
    // let server = tokio::spawn(server::run_server(server_addr));

    // 等待服务器启动
    // tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // 运行客户端
    // client::run_client(server_addr);
    let mut client = rrcp::client::RrcpClient::new(server_addr).await?;
    let config = client.get_config().await?;
    info!("config={:?}", config);
    let sensor_data = SensorData {
        servos: vec![
            ServoStatus { angle: 1.0 },
            ServoStatus { angle: 2.0 },
            ServoStatus { angle: 3.0 },
        ],
        images: vec![Image {
            width: 3,
            height: 4,
            data: vec![1; 3 * 4],
        }],
    };
    for i in 0..10 {
        let action = client.get_action(&sensor_data).await?;
        info!("action={:?}", action);
        sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}
