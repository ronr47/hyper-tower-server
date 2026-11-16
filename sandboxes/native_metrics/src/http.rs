use std::fmt::{self, Write as FmtWrite};
use std::fs::File;
use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct StackBuf<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}

impl<const N: usize> StackBuf<N> {
    pub const fn new() -> Self {
        Self { buf: [0u8; N], len: 0 }
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl<const N: usize> FmtWrite for StackBuf<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let to_copy = bytes.len().min(N.saturating_sub(self.len));
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        if to_copy < bytes.len() { Err(fmt::Error) } else { Ok(()) }
    }
}

fn read_proc_stack<const N: usize>(path: &str, out: &mut [u8; N]) -> usize {
    let Ok(mut file) = File::open(path) else { return 0 };
    let mut total = 0;
    while total < N {
        match file.read(&mut out[total..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
    total
}

fn write_telemetry_json<const N: usize>(out: &mut StackBuf<N>, daemon_ticks: u64) {
    let mut raw = [0u8; 2048];

    let n = read_proc_stack("/proc/uptime", &mut raw);
    let uptime_secs = core::str::from_utf8(&raw[..n])
        .ok()
        .and_then(|s| s.split_whitespace().next())
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let n = read_proc_stack("/proc/loadavg", &mut raw);
    let (l1, l5, l15) = core::str::from_utf8(&raw[..n])
        .ok()
        .map(|s| {
            let mut p = s.split_whitespace();
            (
                p.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
                p.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
                p.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0, 0.0));

    let (mut mem_total, mut mem_avail) = (0u64, 0u64);
    let n = read_proc_stack("/proc/meminfo", &mut raw);
    if let Ok(meminfo) = core::str::from_utf8(&raw[..n]) {
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                mem_total = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                mem_avail = rest.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0);
            }
        }
    }

    let mut host_raw = [0u8; 64];
    let hn = read_proc_stack("/etc/hostname", &mut host_raw);
    let hostname = core::str::from_utf8(&host_raw[..hn])
        .ok()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("Dellamarie");

    let ts_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);

    let _ = write!(
        out,
        "{{\n  \"node\": \"{}\",\n  \"status\": \"ONLINE\",\n  \"timestamp_ms\": {},\n  \"daemon_ticks\": {},\n  \"uptime_secs\": {:.2},\n  \"load_avg\": [{:.2}, {:.2}, {:.2}],\n  \"memory_kb\": {{ \"total\": {}, \"available\": {} }}\n}}",
        hostname, ts_ms, daemon_ticks, uptime_secs, l1, l5, l15, mem_total, mem_avail
    );
}

pub async fn run_http_server(bind_addr: &str, shared_counter: Arc<AtomicU64>) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind_addr).await?;
    println!("[+] Telemetry HTTP/JSON endpoint listening on http://{}", bind_addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        let counter = shared_counter.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            let Ok(n) = socket.read(&mut buf).await else { return };
            if n == 0 { return; }

            let req = core::str::from_utf8(&buf[..n]).unwrap_or("");
            let mut parts = req.lines().next().unwrap_or("").split_whitespace();
            let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));

            let origin = req.lines().find_map(|line| {
                let (k, v) = line.split_once(':')?;
                if k.eq_ignore_ascii_case("Origin") { Some(v.trim()) } else { None }
            }).unwrap_or("https://saltobo.com");

            let allowed = match origin {
                "https://saltobo.com" | "https://www.saltobo.com" | "http://localhost:5173" => origin,
                _ => "https://saltobo.com",
            };

            let mut resp = StackBuf::<1536>::new();
            if method == "OPTIONS" {
                let _ = write!(
                    resp,
                    "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nAccess-Control-Max-Age: 86400\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    allowed
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                return;
            }

            let mut body = StackBuf::<512>::new();
            let status = match (method, path) {
                ("GET", "/telemetry") | ("GET", "/") | ("GET", "/health") => {
                    write_telemetry_json(&mut body, counter.load(Ordering::Relaxed));
                    "200 OK"
                }
                _ => {
                    let _ = body.write_str("{\"error\":\"not_found\"}");
                    "404 Not Found"
                }
            };

            let _ = write!(
                resp,
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: {}\r\nVary: Origin\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status, allowed, body.len
            );
            let _ = socket.write_all(resp.as_bytes()).await;
            let _ = socket.write_all(body.as_bytes()).await;
        });
    }
}
