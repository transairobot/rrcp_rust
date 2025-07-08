use std::sync::Arc;

use quinn::{Connection, RecvStream, SendStream};
use tokio::sync::Mutex;

pub struct StreamPool {
    streams: Arc<Mutex<Vec<(quinn::SendStream, quinn::RecvStream)>>>,
    conn: Arc<Connection>,
}

impl StreamPool {
    pub async fn new(pool_size: usize, conn: quinn::Connection) -> anyhow::Result<Self> {
        let self_ = Self {
            streams: Arc::new(Mutex::new(Vec::with_capacity(pool_size))),
            conn: Arc::new(conn),
        };
        for _ in 0..pool_size {
            let (send, recv) = self_.conn.open_bi().await?;
            self_.streams.lock().await.push((send, recv));
        }
        Ok(self_)
    }

    pub async fn get_bi_stream(&mut self) -> anyhow::Result<(SendStream, RecvStream)> {
        let streams_clone = self.streams.clone();
        let conn_clone = self.conn.clone();
        tokio::spawn(async move {
            // Replenish the pool if needed
            if streams_clone.lock().await.len() < 5 {
                // Example threshold
                let (send, recv) = conn_clone.open_bi().await.unwrap();
                streams_clone.lock().await.push((send, recv));
            }
        });
        let mut streams = self.streams.lock().await;
        if let Some(stream) = streams.pop() {
            Ok(stream)
        } else {
            Err(anyhow::anyhow!("No available send stream"))
        }
    }

    // pub async fn write(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
    //     let (mut send_stream, mut recv_stream) = self.get_send_stream().await?;
    //     send_stream.write_all(&data).await?;
    //     recv_stream.read().await?;
    //     Ok(())
    // }
}
