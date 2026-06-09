use crate::debugbar::{self, RequestMetrics};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

const HOT_RELOAD_PATH: &str = "/.web/hot";
const DEV_CLIENT_PATH: &str = "/.web/dev-client.js";
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Clone)]
pub struct DevServer {
    root: PathBuf,
    clients: Arc<Mutex<Vec<mpsc::Sender<()>>>>,
}

impl DevServer {
    pub fn start(root: PathBuf) -> Self {
        let server = Self {
            root,
            clients: Arc::new(Mutex::new(Vec::new())),
        };
        server.spawn_watcher();
        server
    }

    pub fn is_hot_reload_path(path: &str) -> bool {
        path == HOT_RELOAD_PATH
    }

    pub fn is_dev_client_path(path: &str) -> bool {
        path == DEV_CLIENT_PATH
    }

    pub fn dev_client_script() -> &'static str {
        r#"(() => {
  let retry = 250;

  function connect() {
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(`${protocol}//${location.host}/.web/hot`);

    socket.addEventListener("open", () => {
      retry = 250;
    });

    socket.addEventListener("message", (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message.type === "reload") {
          location.reload();
        }
      } catch {
        location.reload();
      }
    });

    socket.addEventListener("close", () => {
      setTimeout(connect, retry);
      retry = Math.min(retry * 2, 4000);
    });
  }

  connect();
})();
"#
    }

    pub fn inject_dev_tools(html: &str, metrics: Option<&RequestMetrics>) -> String {
        let script = r#"<script src="/.web/dev-client.js"></script>"#;
        if html.contains(script) {
            return html.to_string();
        }

        let mut fragment = String::new();
        if let Some(metrics) = metrics {
            fragment.push_str(&debugbar::render_html(metrics));
        }
        fragment.push_str(script);

        if let Some(index) = html.rfind("</body>") {
            let mut injected = String::with_capacity(html.len() + fragment.len());
            injected.push_str(&html[..index]);
            injected.push_str(&fragment);
            injected.push_str(&html[index..]);
            injected
        } else {
            let mut injected = String::with_capacity(html.len() + fragment.len());
            injected.push_str(html);
            injected.push_str(&fragment);
            injected
        }
    }

    pub fn handle_hot_reload(&self, mut stream: TcpStream, request: &str) -> Result<(), String> {
        let Some(key) = websocket_key(request) else {
            return write_bad_websocket_response(&mut stream);
        };

        let accept_key = websocket_accept_key(&key);
        let response = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {accept_key}\r\n\
             \r\n"
        );
        stream
            .write_all(response.as_bytes())
            .map_err(|error| error.to_string())?;

        let (sender, receiver) = mpsc::channel();
        self.clients
            .lock()
            .map_err(|_| "hot reload clients lock poisoned".to_string())?
            .push(sender);

        for _ in receiver {
            if write_text_frame(&mut stream, r#"{"type":"reload"}"#).is_err() {
                break;
            }
        }

        Ok(())
    }

    fn spawn_watcher(&self) {
        let root = self.root.clone();
        let clients = Arc::clone(&self.clients);

        thread::spawn(move || {
            let mut previous = snapshot_project_files(&root).unwrap_or_default();

            loop {
                thread::sleep(Duration::from_millis(300));
                let Ok(next) = snapshot_project_files(&root) else {
                    continue;
                };

                if next != previous {
                    previous = next;
                    thread::sleep(Duration::from_millis(100));
                    broadcast_reload(&clients);
                }
            }
        });
    }
}

fn broadcast_reload(clients: &Arc<Mutex<Vec<mpsc::Sender<()>>>>) {
    if let Ok(mut clients) = clients.lock() {
        clients.retain(|client| client.send(()).is_ok());
    }
}

fn snapshot_project_files(root: &Path) -> Result<BTreeMap<PathBuf, Option<SystemTime>>, String> {
    let mut files = BTreeMap::new();
    collect_watch_files(&root.join("app"), root, &mut files)?;
    collect_watch_files(&root.join("public"), root, &mut files)?;

    let config = root.join("web.config");
    if config.exists() {
        files.insert(relative_to_root(root, &config), modified_time(&config));
    }

    Ok(files)
}

fn collect_watch_files(
    directory: &Path,
    root: &Path,
    files: &mut BTreeMap<PathBuf, Option<SystemTime>>,
) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_watch_files(&path, root, files)?;
        } else {
            files.insert(relative_to_root(root, &path), modified_time(&path));
        }
    }

    Ok(())
}

fn relative_to_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn websocket_key(request: &str) -> Option<String> {
    for line in request.lines() {
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("Sec-WebSocket-Key") {
                return Some(value.trim().to_string());
            }
        }
    }

    None
}

fn write_bad_websocket_response(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .write_all(
            b"HTTP/1.1 400 Bad Request\r\nContent-Type: text/plain\r\nContent-Length: 21\r\n\r\nBad WebSocket Request",
        )
        .map_err(|error| error.to_string())
}

fn write_text_frame(stream: &mut TcpStream, payload: &str) -> Result<(), String> {
    let bytes = payload.as_bytes();
    let mut frame = Vec::with_capacity(bytes.len() + 10);
    frame.push(0x81);

    if bytes.len() < 126 {
        frame.push(bytes.len() as u8);
    } else if bytes.len() <= u16::MAX as usize {
        frame.push(126);
        frame.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    } else {
        frame.push(127);
        frame.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    }

    frame.extend_from_slice(bytes);
    stream.write_all(&frame).map_err(|error| error.to_string())
}

fn websocket_accept_key(key: &str) -> String {
    let mut input = Vec::with_capacity(key.len() + WS_GUID.len());
    input.extend_from_slice(key.as_bytes());
    input.extend_from_slice(WS_GUID.as_bytes());
    base64_encode(&sha1(&input))
}

fn sha1(input: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xefcdab89;
    let mut h2: u32 = 0x98badcfe;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xc3d2e1f0;

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut words = [0u32; 80];
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (index, word) in words.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut output = [0u8; 20];
    for (index, word) in [h0, h1, h2, h3, h4].iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);

        encoded.push(TABLE[(b0 >> 2) as usize] as char);
        encoded.push(TABLE[(((b0 & 0b00000011) << 4) | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            encoded.push(TABLE[(((b1 & 0b00001111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            encoded.push('=');
        }

        if chunk.len() > 2 {
            encoded.push(TABLE[(b2 & 0b00111111) as usize] as char);
        } else {
            encoded.push('=');
        }
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{base64_encode, websocket_accept_key, DevServer};
    use crate::debugbar::RequestMetrics;
    use std::time::Duration;

    #[test]
    fn injects_client_before_body_close() {
        let html = "<html><body><main>Hello</main></body></html>";

        assert_eq!(
            DevServer::inject_dev_tools(html, None),
            r#"<html><body><main>Hello</main><script src="/.web/dev-client.js"></script></body></html>"#
        );
    }

    #[test]
    fn appends_client_when_body_is_missing() {
        let html = "<main>Hello</main>";

        assert_eq!(
            DevServer::inject_dev_tools(html, None),
            r#"<main>Hello</main><script src="/.web/dev-client.js"></script>"#
        );
    }

    #[test]
    fn injects_debug_bar_before_hot_reload_script() {
        let html = "<html><body><main>Hello</main></body></html>";
        let mut metrics = RequestMetrics::new("/");
        metrics.push("Render", Duration::from_millis(2), None);
        metrics.set_total(Duration::from_millis(2));

        let injected = DevServer::inject_dev_tools(html, Some(&metrics));

        let debug_index = injected.find("webscript-debugbar").expect("debug bar");
        let script_index = injected
            .find(r#"<script src="/.web/dev-client.js"></script>"#)
            .expect("dev client script");
        assert!(debug_index < script_index);
    }

    #[test]
    fn appends_debug_bar_and_client_when_body_is_missing() {
        let html = "<main>Hello</main>";
        let mut metrics = RequestMetrics::new("/posts");
        metrics.set_total(Duration::from_millis(1));

        let injected = DevServer::inject_dev_tools(html, Some(&metrics));

        assert!(injected.contains("webscript-debugbar"));
        assert!(injected.ends_with(r#"<script src="/.web/dev-client.js"></script>"#));
    }

    #[test]
    fn computes_websocket_accept_key() {
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn encodes_base64_padding() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }
}
