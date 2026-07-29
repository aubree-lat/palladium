
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use super::bridge::Bridge;
use super::protocol::{self, error_codes, PacketType, CLOSE_UNSUPPORTED};
use super::RpcEvent;

static NEXT_SOCKET_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct Ctx {
    pub bridge: Bridge,
    pub events: Option<mpsc::UnboundedSender<RpcEvent>>,
}

#[cfg(unix)]
pub async fn serve(ctx: Ctx) -> Result<()> {
    use tokio::net::{UnixListener, UnixStream};

    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("TMPDIR"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);

    let mut chosen = None;
    for i in 0..10 {
        let path = dir.join(format!("discord-ipc-{i}"));

        if UnixStream::connect(&path).await.is_ok() {
            log::debug!("{} is taken, trying the next slot", path.display());
            continue;
        }

        let _ = std::fs::remove_file(&path);
        match UnixListener::bind(&path) {
            Ok(listener) => {
                chosen = Some((listener, path));
                break;
            }
            Err(e) => log::debug!("could not bind {}: {e}", path.display()),
        }
    }

    let Some((listener, path)) = chosen else {
        bail!("discord-ipc-0 through discord-ipc-9 are all in use");
    };
    log::info!("arRPC listening on {}", path.display());

    let cleanup = path.clone();
    let _guard = scopeguard(move || {
        let _ = std::fs::remove_file(&cleanup);
    });

    loop {
        let (stream, _) = listener.accept().await.context("accepting IPC connection")?;
        let ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = session(stream, ctx).await {
                log::debug!("IPC session ended: {e}");
            }
        });
    }
}

#[cfg(windows)]
pub async fn serve(ctx: Ctx) -> Result<()> {
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    let mut chosen = None;
    for i in 0..10 {
        let name = format!(r"\\.\pipe\discord-ipc-{i}");

        if ClientOptions::new().open(&name).is_ok() {
            log::debug!("{name} is taken, trying the next slot");
            continue;
        }

        match ServerOptions::new().first_pipe_instance(true).create(&name) {
            Ok(server) => {
                chosen = Some((server, name));
                break;
            }
            Err(e) => log::debug!("could not create {name}: {e}"),
        }
    }

    let Some((mut server, name)) = chosen else {
        bail!("discord-ipc-0 through discord-ipc-9 are all in use");
    };
    log::info!("arRPC listening on {name}");

    loop {
        server.connect().await.context("accepting IPC connection")?;
        let connected = server;
        server = ServerOptions::new()
            .create(&name)
            .context("creating the next pipe instance")?;

        let ctx = ctx.clone();
        tokio::spawn(async move {
            if let Err(e) = session(connected, ctx).await {
                log::debug!("IPC session ended: {e}");
            }
        });
    }
}

#[cfg(unix)]
fn scopeguard<F: FnOnce()>(f: F) -> impl Drop {
    struct Guard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for Guard<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
    Guard(Some(f))
}

struct Session {
    socket_id: String,
    client_id: String,
    last_pid: Value,
    handshook: bool,
}

async fn session<S>(mut stream: S, ctx: Ctx) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut sess = Session {
        socket_id: NEXT_SOCKET_ID.fetch_add(1, Ordering::Relaxed).to_string(),
        client_id: String::new(),
        last_pid: Value::Null,
        handshook: false,
    };

    log::info!("new RPC connection (socket {})", sess.socket_id);

    let result = run(&mut stream, &mut sess, &ctx).await;

    if sess.handshook {
        ctx.bridge
            .send(&sess.socket_id, Value::Null, sess.last_pid.clone())
            .await;
    }
    log::info!("RPC connection closed (socket {})", sess.socket_id);

    result
}

async fn run<S>(stream: &mut S, sess: &mut Session, ctx: &Ctx) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    while let Some((kind, payload)) = protocol::read_packet(stream).await? {
        match kind {
            PacketType::Ping => {
                protocol::write_packet(stream, PacketType::Pong, &payload).await?;
            }
            PacketType::Pong => {}
            PacketType::Close => break,

            PacketType::Handshake => {
                if sess.handshook {
                    close(stream, CLOSE_UNSUPPORTED, "already handshook").await?;
                    break;
                }

                let version = payload
                    .get("v")
                    .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
                    .unwrap_or(1);
                if version != 1 {
                    log::warn!("unsupported RPC version {version}");
                    close(stream, error_codes::INVALID_VERSION, "unsupported version").await?;
                    break;
                }

                let client_id = payload
                    .get("client_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if client_id.is_empty() {
                    close(stream, error_codes::INVALID_CLIENTID, "client id required").await?;
                    break;
                }

                sess.client_id = client_id;
                sess.handshook = true;
                log::info!("handshake from client {} ", sess.client_id);

                protocol::write_packet(stream, PacketType::Frame, &ready_payload()).await?;
            }

            PacketType::Frame => {
                if !sess.handshook {
                    close(stream, CLOSE_UNSUPPORTED, "need to handshake first").await?;
                    break;
                }
                handle_command(stream, sess, ctx, &payload).await?;
            }
        }
    }

    Ok(())
}

async fn close<S>(stream: &mut S, code: i32, message: &str) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    protocol::write_packet(
        stream,
        PacketType::Close,
        &json!({ "code": code, "message": message }),
    )
    .await
}

fn ready_payload() -> Value {
    json!({
        "cmd": "DISPATCH",
        "data": {
            "v": 1,
            "config": {
                "cdn_host": "cdn.discordapp.com",
                "api_endpoint": "//discord.com/api",
                "environment": "production"
            },
            "user": {
                "id": "1045800378228281345",
                "username": "arrpc",
                "discriminator": "0",
                "global_name": "arRPC",
                "avatar": "cfefa4d9839fb4bdf030f91c2a13e95c",
                "avatar_decoration_data": null,
                "bot": false,
                "flags": 0,
                "premium_type": 0
            }
        },
        "evt": "READY",
        "nonce": null
    })
}

async fn handle_command<S>(
    stream: &mut S,
    sess: &mut Session,
    ctx: &Ctx,
    msg: &Value,
) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    let cmd = msg.get("cmd").and_then(Value::as_str).unwrap_or_default();
    let nonce = msg.get("nonce").cloned().unwrap_or(Value::Null);
    let args = msg.get("args").cloned().unwrap_or(Value::Null);

    match cmd {
        "SET_ACTIVITY" => {
            let pid = args.get("pid").cloned().unwrap_or(Value::Null);
            if !pid.is_null() {
                sess.last_pid = pid.clone();
            }

            let activity = args.get("activity").cloned().unwrap_or(Value::Null);
            if activity.is_null() {
                ctx.bridge.send(&sess.socket_id, Value::Null, pid).await;
                reply(stream, cmd, Value::Null, &nonce).await?;
                return Ok(());
            }

            let translated = translate_activity(&activity, &sess.client_id);
            ctx.bridge.send(&sess.socket_id, translated, pid).await;

            let mut echo = activity.clone();
            if let Some(obj) = echo.as_object_mut() {
                obj.insert("name".into(), json!(""));
                obj.insert("application_id".into(), json!(sess.client_id));
                obj.insert("type".into(), json!(0));
            }
            reply(stream, cmd, echo, &nonce).await?;
        }

        "CONNECTIONS_CALLBACK" => {
            error_reply(stream, cmd, json!({ "code": 1000 }), &nonce).await?;
        }

        "INVITE_BROWSER" | "GUILD_TEMPLATE_BROWSER" => {
            let code = args.get("code").and_then(Value::as_str).unwrap_or_default();
            let is_invite = cmd == "INVITE_BROWSER";

            if code.is_empty() {
                let kind = if is_invite { "invite" } else { "guild template" };
                error_reply(
                    stream,
                    cmd,
                    json!({
                        "code": if is_invite { 4011 } else { 4017 },
                        "message": format!("Invalid {kind} id: {code}")
                    }),
                    &nonce,
                )
                .await?;
                return Ok(());
            }

            emit(ctx, RpcEvent::Invite {
                code: code.to_string(),
                is_invite,
            });
            reply(stream, cmd, json!({ "code": code }), &nonce).await?;
        }

        "DEEP_LINK" => {
            emit(ctx, RpcEvent::DeepLink { args: args.clone() });
            reply(stream, cmd, Value::Null, &nonce).await?;
        }

        other => {
            log::debug!("ignoring unhandled RPC command {other}");
        }
    }

    Ok(())
}

fn emit(ctx: &Ctx, event: RpcEvent) {
    if let Some(tx) = &ctx.events {
        let _ = tx.send(event);
    }
}

async fn reply<S>(stream: &mut S, cmd: &str, data: Value, nonce: &Value) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    protocol::write_packet(
        stream,
        PacketType::Frame,
        &json!({ "cmd": cmd, "data": data, "evt": null, "nonce": nonce }),
    )
    .await
}

async fn error_reply<S>(stream: &mut S, cmd: &str, data: Value, nonce: &Value) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    protocol::write_packet(
        stream,
        PacketType::Frame,
        &json!({ "cmd": cmd, "data": data, "evt": "ERROR", "nonce": nonce }),
    )
    .await
}

fn translate_activity(activity: &Value, client_id: &str) -> Value {
    let mut out = serde_json::Map::new();
    out.insert("application_id".into(), json!(client_id));
    out.insert("type".into(), json!(0));

    let mut metadata = serde_json::Map::new();
    let mut buttons_labels = None;

    if let Some(buttons) = activity.get("buttons").and_then(Value::as_array) {
        let urls: Vec<Value> = buttons
            .iter()
            .map(|b| b.get("url").cloned().unwrap_or(Value::Null))
            .collect();
        let labels: Vec<Value> = buttons
            .iter()
            .map(|b| b.get("label").cloned().unwrap_or(Value::Null))
            .collect();
        metadata.insert("button_urls".into(), Value::Array(urls));
        buttons_labels = Some(Value::Array(labels));
    }

    out.insert("metadata".into(), Value::Object(metadata));

    let instance = activity
        .get("instance")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    out.insert("flags".into(), json!(if instance { 1 } else { 0 }));

    if let Some(obj) = activity.as_object() {
        for (k, v) in obj {
            out.insert(k.clone(), v.clone());
        }
    }

    if let Some(ts) = out.get_mut("timestamps").and_then(Value::as_object_mut) {
        normalise_timestamps(ts);
    }

    if let Some(labels) = buttons_labels {
        out.insert("buttons".into(), labels);
    }

    Value::Object(out)
}

fn normalise_timestamps(ts: &mut serde_json::Map<String, Value>) {
    let now_digits = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string().len())
        .unwrap_or(13);

    for value in ts.values_mut() {
        let Some(n) = value.as_f64() else { continue };
        if n <= 0.0 {
            continue;
        }
        let digits = (n.trunc() as u64).to_string().len();
        if now_digits.saturating_sub(digits) > 2 {
            *value = json!((n * 1000.0).floor() as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_timestamps_are_promoted_to_millis() {
        let activity = json!({
            "details": "in a match",
            "timestamps": { "start": 1_700_000_000u64 }
        });
        let out = translate_activity(&activity, "123");
        assert_eq!(out["timestamps"]["start"], json!(1_700_000_000_000u64));
    }

    #[test]
    fn millisecond_timestamps_are_left_alone() {
        let activity = json!({ "timestamps": { "start": 1_700_000_000_000u64 } });
        let out = translate_activity(&activity, "123");
        assert_eq!(out["timestamps"]["start"], json!(1_700_000_000_000u64));
    }

    #[test]
    fn buttons_split_into_labels_and_metadata_urls() {
        let activity = json!({
            "buttons": [{ "label": "Website", "url": "https://example.com" }]
        });
        let out = translate_activity(&activity, "123");
        assert_eq!(out["buttons"], json!(["Website"]));
        assert_eq!(
            out["metadata"]["button_urls"],
            json!(["https://example.com"])
        );
    }

    #[test]
    fn client_id_becomes_the_application_id() {
        let out = translate_activity(&json!({ "details": "hi" }), "999");
        assert_eq!(out["application_id"], json!("999"));
        assert_eq!(out["type"], json!(0));
        assert_eq!(out["details"], json!("hi"));
    }

    #[test]
    fn instance_flag_is_set() {
        let out = translate_activity(&json!({ "instance": true }), "1");
        assert_eq!(out["flags"], json!(1));
    }
}
