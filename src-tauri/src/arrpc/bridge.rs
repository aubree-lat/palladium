
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures_util::SinkExt;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
pub struct Bridge {
    tx: broadcast::Sender<String>,
    last: Arc<Mutex<HashMap<String, Value>>>,
}

impl Bridge {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(64);
        Self {
            tx,
            last: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn send(&self, socket_id: &str, activity: Value, pid: Value) {
        let msg = json!({
            "activity": activity,
            "pid": pid,
            "socketId": socket_id,
        });

        {
            let mut guard = self.last.lock().await;
            if msg["activity"].is_null() {
                guard.remove(socket_id);
            } else {
                guard.insert(socket_id.to_string(), msg.clone());
            }
        }

        let _ = self.tx.send(msg.to_string());
    }

    pub async fn bind(port: u16) -> Result<TcpListener> {
        TcpListener::bind(("127.0.0.1", port))
            .await
            .with_context(|| format!("binding arRPC bridge on 127.0.0.1:{port}"))
    }

    pub async fn serve(self, listener: TcpListener) -> Result<()> {
        if let Ok(addr) = listener.local_addr() {
            log::info!("arRPC bridge listening on ws://{addr}");
        }

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("bridge accept failed: {e}");
                    continue;
                }
            };

            let rx = self.tx.subscribe();
            let last = self.last.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_ws(stream, rx, last).await {
                    log::debug!("bridge client ended: {e}");
                }
            });
        }
    }
}

async fn handle_ws(
    stream: tokio::net::TcpStream,
    mut rx: broadcast::Receiver<String>,
    last: Arc<Mutex<HashMap<String, Value>>>,
) -> Result<()> {
    let mut ws = tokio_tungstenite::accept_async(stream).await?;
    log::info!("client mod connected to arRPC bridge");

    {
        let guard = last.lock().await;
        for msg in guard.values() {
            ws.send(Message::text(msg.to_string())).await?;
        }
    }

    loop {
        match rx.recv().await {
            Ok(msg) => ws.send(Message::text(msg)).await?,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("bridge client lagged {n} messages");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }

    Ok(())
}
