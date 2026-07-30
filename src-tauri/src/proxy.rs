use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::{HeaderName, HeaderValue};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rand::Rng;
use tokio::net::TcpListener;

pub struct Proxy {
    pub port: u16,
    pub token: String,
}

const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

const STRIPPED: [&str; 6] = [
    "content-security-policy",
    "content-security-policy-report-only",
    "content-encoding",
    "content-length",
    "access-control-allow-origin",
    "x-frame-options",
];

fn random_token() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn cors<T>(mut res: Response<T>) -> Response<T> {
    let h = res.headers_mut();
    h.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    h.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("*"),
    );
    h.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
    );
    h.insert("access-control-expose-headers", HeaderValue::from_static("*"));
    res
}

fn empty(status: StatusCode) -> Response<Full<Bytes>> {
    let mut res = Response::new(Full::new(Bytes::new()));
    *res.status_mut() = status;
    cors(res)
}

fn clipboard_image() -> Response<Full<Bytes>> {
    if !crate::clipboard::has_main_thread() {
        return empty(StatusCode::SERVICE_UNAVAILABLE);
    }
    match crate::clipboard::image_png() {
        Some(png) => {
            log::debug!("served {} bytes of clipboard image", png.len());
            let mut res = Response::new(Full::new(Bytes::from(png)));
            res.headers_mut()
                .insert("content-type", HeaderValue::from_static("image/png"));
            res.headers_mut()
                .insert("cache-control", HeaderValue::from_static("no-store"));
            cors(res)
        }
        None => empty(StatusCode::NO_CONTENT),
    }
}

async fn handle(
    req: Request<Incoming>,
    client: Arc<reqwest::Client>,
    token: Arc<String>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if req.method() == Method::OPTIONS {
        return Ok(empty(StatusCode::NO_CONTENT));
    }

    let path = req.uri().path().trim_start_matches('/').to_string();
    let (path_token, rest) = match path.split_once('/') {
        Some((t, r)) => (t, r),
        None => (path.as_str(), ""),
    };

    if path_token != token.as_str() {
        log::warn!("rejected local request with a bad token");
        return Ok(empty(StatusCode::FORBIDDEN));
    }

    if rest == "clipboard-image" {
        return Ok(clipboard_image());
    }

    if rest == "log" {
        let msg = url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .find(|(k, _)| k == "m")
            .map(|(_, v)| v.to_string())
            .unwrap_or_default();
        log::info!("[page] {}", msg.chars().take(300).collect::<String>());
        return Ok(empty(StatusCode::NO_CONTENT));
    }

    if rest == "zoom" {
        let step = url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
            .find(|(k, _)| k == "step")
            .and_then(|(_, v)| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        crate::apply_zoom_step(step);
        return Ok(empty(StatusCode::NO_CONTENT));
    }

    let query = req.uri().query().unwrap_or("");
    let target = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.to_string());

    let Some(target) = target else {
        return Ok(empty(StatusCode::BAD_REQUEST));
    };

    let parsed = match url::Url::parse(&target) {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u,
        _ => return Ok(empty(StatusCode::BAD_REQUEST)),
    };

    let method = reqwest::Method::from_bytes(req.method().as_str().as_bytes())
        .unwrap_or(reqwest::Method::GET);

    let mut upstream = client.request(method, parsed.clone());
    for (name, value) in req.headers() {
        let n = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&n.as_str()) || n == "host" || n == "origin" || n == "referer" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            upstream = upstream.header(name.as_str(), v);
        }
    }

    let body = match req.into_body().collect().await {
        Ok(c) => c.to_bytes(),
        Err(_) => return Ok(empty(StatusCode::BAD_REQUEST)),
    };
    if !body.is_empty() {
        upstream = upstream.body(body.to_vec());
    }

    let response = match upstream.send().await {
        Ok(r) => r,
        Err(e) => {
            log::warn!("proxy request to {} failed: {e}", parsed.host_str().unwrap_or("?"));
            return Ok(empty(StatusCode::BAD_GATEWAY));
        }
    };

    let status = response.status();
    let headers = response.headers().clone();
    let payload = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return Ok(empty(StatusCode::BAD_GATEWAY)),
    };

    let mut out = Response::new(Full::new(payload));
    *out.status_mut() = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK);
    for (name, value) in headers.iter() {
        let n = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&n.as_str()) || STRIPPED.contains(&n.as_str()) {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            HeaderName::from_bytes(name.as_str().as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out.headers_mut().insert(hn, hv);
        }
    }

    log::debug!("proxied {} -> {}", parsed, status);
    Ok(cors(out))
}

pub fn spawn() -> Result<Proxy> {
    let token = random_token();
    let (tx, rx) = std::sync::mpsc::channel::<Result<u16>>();

    let thread_token = token.clone();
    std::thread::Builder::new()
        .name("palladium-proxy".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .thread_name("proxy")
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(Err(e.into()));
                    return;
                }
            };

            runtime.block_on(async move {
                let listener = match TcpListener::bind(("127.0.0.1", 0)).await {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        return;
                    }
                };

                let port = match listener.local_addr() {
                    Ok(a) => a.port(),
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        return;
                    }
                };
                let _ = tx.send(Ok(port));

                let client = match reqwest::Client::builder()
                    .timeout(Duration::from_secs(30))
                    .build()
                {
                    Ok(c) => Arc::new(c),
                    Err(e) => {
                        log::error!("proxy client init failed: {e}");
                        return;
                    }
                };
                let token = Arc::new(thread_token);

                loop {
                    let Ok((stream, _)) = listener.accept().await else {
                        continue;
                    };
                    let client = client.clone();
                    let token = token.clone();
                    tokio::spawn(async move {
                        let service = service_fn(move |req| {
                            handle(req, client.clone(), token.clone())
                        });
                        if let Err(e) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .await
                        {
                            log::debug!("proxy connection ended: {e}");
                        }
                    });
                }
            });
        })?;

    let port = rx
        .recv_timeout(Duration::from_secs(5))
        .context("proxy thread did not report a port")??;

    log::info!("csp bypass proxy listening on http://127.0.0.1:{port}");
    log::trace!("local endpoint base: http://127.0.0.1:{port}/{token}");
    Ok(Proxy { port, token })
}
