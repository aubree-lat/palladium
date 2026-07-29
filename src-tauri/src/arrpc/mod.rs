
pub mod bridge;
pub mod ipc;
pub mod protocol;

use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum RpcEvent {
    Invite { code: String, is_invite: bool },
    DeepLink { args: Value },
}

pub fn spawn(bridge_port: u16) -> Result<mpsc::UnboundedReceiver<RpcEvent>> {
    let (tx, rx) = mpsc::unbounded_channel();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("arrpc")
        .build()?;

    std::thread::Builder::new()
        .name("arrpc".into())
        .spawn(move || {
            runtime.block_on(async move {
                let listener = match bridge::Bridge::bind(bridge_port).await {
                    Ok(l) => l,
                    Err(e) if is_addr_in_use(&e) => {
                        log::warn!(
                            "port {bridge_port} is already in use, so another arRPC server is \
                             running. Leaving Rich Presence to it and not starting the built-in \
                             one. Set arrpc_enabled to false to silence this."
                        );
                        return;
                    }
                    Err(e) => {
                        log::error!("could not start the arRPC bridge: {e:#}");
                        return;
                    }
                };

                let bridge = bridge::Bridge::new();

                let bridge_task = {
                    let bridge = bridge.clone();
                    tokio::spawn(async move {
                        if let Err(e) = bridge.serve(listener).await {
                            log::error!("arRPC bridge stopped: {e:#}");
                        }
                    })
                };

                let ctx = ipc::Ctx {
                    bridge,
                    events: Some(tx),
                };
                let ipc_task = tokio::spawn(async move {
                    if let Err(e) = ipc::serve(ctx).await {
                        log::error!("arRPC IPC server stopped: {e:#}");
                    }
                });

                let _ = tokio::join!(bridge_task, ipc_task);
            });
        })?;

    Ok(rx)
}

fn is_addr_in_use(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| e.kind() == std::io::ErrorKind::AddrInUse)
}
