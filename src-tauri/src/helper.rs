//! The privileged helper behind the native macOS app.
//!
//! The app asks for the administrator password once at launch and starts
//! `moondisk-helper --listen <socket> --owner <uid>` as root (see
//! `macos/Sources/MoonDisk/Helper.swift`). The helper accepts exactly one
//! connection on that socket — only the owner can open it — and serves
//! it until the app disconnects:
//!
//! ```text
//! → {"id":1,"cmd":"listDisks"}
//! ← {"id":1,"ok":[…]}
//! → {"id":2,"cmd":"flash","imagePath":"…","fd":true,"diskId":"/dev/disk4","mode":"copy","verify":true}
//! ← {"event":"flashProgress","data":{…}}   (repeated)
//! ← {"id":2,"ok":null}                      or {"id":2,"err":"…"}
//! ```
//!
//! One JSON object per line. A request with `"fd":true` uses a file the app
//! opened and sent over the socket just before (`SCM_RIGHTS`): macOS'
//! privacy protection may stop even root from opening files in the
//! user's Downloads folder, while the app may read what the user picked.
//! Disk operations go through `service`, exactly like the Tauri app's.

use crate::flash::{self, WriteMode};
use crate::operations::OperationRequest;
use crate::service;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::fs::{File, Permissions};
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long to wait for the app to connect before giving up.
const ACCEPT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
enum Command {
    Ping,
    ListDisks,
    Execute {
        request: OperationRequest,
        confirmed: bool,
    },
    ImageInfo {
        path: String,
        #[serde(default)]
        fd: bool,
    },
    Flash {
        image_path: String,
        #[serde(default)]
        fd: bool,
        disk_id: String,
        mode: WriteMode,
        verify: bool,
    },
    CancelFlash,
}

/// Reads lines from the socket, keeping any file descriptors that arrive.
struct Reader {
    stream: UnixStream,
    buf: Vec<u8>,
    fds: VecDeque<OwnedFd>,
}

impl Reader {
    fn read_line(&mut self) -> io::Result<Option<String>> {
        loop {
            if let Some(end) = self.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buf.drain(..=end).collect();
                return Ok(Some(String::from_utf8_lossy(&line).trim().to_string()));
            }
            let mut chunk = [0u8; 64 * 1024];
            let (n, fds) = crate::fdpass::recv(&self.stream, &mut chunk)?;
            self.fds.extend(fds);
            if n == 0 {
                return Ok(None);
            }
            self.buf.extend_from_slice(&chunk[..n]);
        }
    }

    /// The file the app sent along with the current request.
    fn take_file(&mut self) -> Result<File, String> {
        self.fds
            .pop_front()
            .map(File::from)
            .ok_or_else(|| "the app didn't hand over the image file".to_string())
    }
}

type Writer = Arc<Mutex<UnixStream>>;

fn send(writer: &Writer, message: &Value) {
    let Ok(mut stream) = writer.lock() else {
        return;
    };
    let mut line = message.to_string();
    line.push('\n');
    // If the app is gone there's nobody left to tell.
    let _ = stream.write_all(line.as_bytes());
}

fn reply(writer: &Writer, id: &Value, result: Result<Value, String>) {
    let message = match result {
        Ok(ok) => json!({ "id": id, "ok": ok }),
        Err(err) => json!({ "id": id, "err": err }),
    };
    send(writer, &message);
}

fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

/// Serves one connection until the other side closes it.
pub fn serve(stream: UnixStream) -> io::Result<()> {
    let writer: Writer = Arc::new(Mutex::new(stream.try_clone()?));
    let mut reader = Reader {
        stream,
        buf: Vec::new(),
        fds: VecDeque::new(),
    };
    let busy = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    let mut worker: Option<JoinHandle<()>> = None;

    while let Some(line) = reader.read_line()? {
        if line.is_empty() {
            continue;
        }
        let mut value: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                reply(&writer, &Value::Null, Err(format!("invalid request: {e}")));
                continue;
            }
        };
        let id = value
            .as_object_mut()
            .and_then(|o| o.remove("id"))
            .unwrap_or(Value::Null);
        let command: Command = match serde_json::from_value(value) {
            Ok(c) => c,
            Err(e) => {
                reply(&writer, &id, Err(format!("invalid request: {e}")));
                continue;
            }
        };

        match command {
            Command::Ping => reply(
                &writer,
                &id,
                Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "platform": service::platform_name(),
                    "root": is_root(),
                })),
            ),
            Command::ListDisks => reply(
                &writer,
                &id,
                service::list_disks().map(|disks| json!(disks)),
            ),
            Command::Execute { request, confirmed } => {
                let result = if busy.load(Ordering::SeqCst) {
                    Err("an image is being written to a USB drive — wait until it finishes".into())
                } else {
                    service::execute(&request, confirmed).map(|_| json!({ "applied": true }))
                };
                reply(&writer, &id, result);
            }
            Command::ImageInfo { path, fd } => {
                let file = if fd {
                    reader.take_file()
                } else {
                    File::open(&path).map_err(|e| format!("{path}: {e}"))
                };
                let result = file.and_then(|mut f| {
                    flash::image_info_of(&mut f, Path::new(&path))
                        .map(|info| json!(info))
                        .map_err(|e| e.to_string())
                });
                reply(&writer, &id, result);
            }
            Command::Flash {
                image_path,
                fd,
                disk_id,
                mode,
                verify,
            } => {
                let file = if fd {
                    reader.take_file()
                } else {
                    File::open(&image_path).map_err(|e| format!("{image_path}: {e}"))
                };
                let mut image = match file {
                    Ok(f) => f,
                    Err(e) => {
                        reply(&writer, &id, Err(e));
                        continue;
                    }
                };
                if busy.swap(true, Ordering::SeqCst) {
                    reply(
                        &writer,
                        &id,
                        Err("another image is already being written".into()),
                    );
                    continue;
                }
                if let Some(done) = worker.take() {
                    let _ = done.join();
                }
                cancel.store(false, Ordering::SeqCst);
                let (writer, busy, cancel) = (writer.clone(), busy.clone(), cancel.clone());
                worker = Some(std::thread::spawn(move || {
                    let result =
                        service::flash(&mut image, &disk_id, mode, verify, &cancel, |progress| {
                            send(
                                &writer,
                                &json!({ "event": "flashProgress", "data": progress }),
                            );
                        });
                    busy.store(false, Ordering::SeqCst);
                    reply(&writer, &id, result.map(|_| Value::Null));
                }));
            }
            Command::CancelFlash => {
                cancel.store(true, Ordering::SeqCst);
                reply(&writer, &id, Ok(Value::Null));
            }
        }
    }

    // The app is gone: stop a running write, but let it finish cleanly
    // (unmounting/re-reading the drive) before exiting.
    cancel.store(true, Ordering::SeqCst);
    if let Some(w) = worker {
        let _ = w.join();
    }
    Ok(())
}

/// Creates the socket at `path` (only `owner` may use it), serves the
/// first connection and exits.
pub fn listen(path: &Path, owner: u32) -> io::Result<()> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, Permissions::from_mode(0o600))?;
    std::os::unix::fs::chown(path, Some(owner), None)?;
    listener.set_nonblocking(true)?;

    let deadline = Instant::now() + ACCEPT_TIMEOUT;
    let stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = std::fs::remove_file(path);
                return Err(e);
            }
        }
    };
    drop(listener);
    let _ = std::fs::remove_file(path);
    stream.set_nonblocking(false)?;
    serve(stream)
}

/// `--listen <socket> --owner <uid>`
pub fn run_from_args(args: &[String]) -> Result<(), String> {
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .ok_or_else(|| {
                format!("usage: moondisk-helper --listen <socket> --owner <uid> (missing {flag})")
            })
    };
    let path = value("--listen")?;
    let owner: u32 = value("--owner")?
        .parse()
        .map_err(|_| "--owner must be a user id".to_string())?;
    listen(Path::new(path), owner).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    struct Client {
        stream: UnixStream,
        lines: BufReader<UnixStream>,
        server: Option<JoinHandle<io::Result<()>>>,
    }

    impl Client {
        fn start() -> Client {
            let (client, server) = UnixStream::pair().unwrap();
            let handle = std::thread::spawn(move || serve(server));
            Client {
                lines: BufReader::new(client.try_clone().unwrap()),
                stream: client,
                server: Some(handle),
            }
        }

        fn send(&mut self, line: &str) {
            self.stream.write_all(line.as_bytes()).unwrap();
            self.stream.write_all(b"\n").unwrap();
        }

        fn next(&mut self) -> Value {
            let mut line = String::new();
            self.lines.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        }

        /// The next reply, skipping progress events.
        fn reply(&mut self) -> Value {
            loop {
                let v = self.next();
                if v.get("event").is_none() {
                    return v;
                }
            }
        }

        fn finish(mut self) {
            self.stream.shutdown(std::net::Shutdown::Both).unwrap();
            self.server.take().unwrap().join().unwrap().unwrap();
        }
    }

    fn fixture(name: &str) -> String {
        format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn answers_ping_with_version_and_platform() {
        let mut c = Client::start();
        c.send(r#"{"id":1,"cmd":"ping"}"#);
        let r = c.reply();
        assert_eq!(r["id"], 1);
        assert_eq!(r["ok"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(r["ok"]["platform"], service::platform_name());
        c.finish();
    }

    #[test]
    fn reports_bad_requests_without_stopping() {
        let mut c = Client::start();
        c.send("not json");
        assert!(c.reply()["err"]
            .as_str()
            .unwrap()
            .contains("invalid request"));
        c.send(r#"{"id":7,"cmd":"format everything"}"#);
        let r = c.reply();
        assert_eq!(r["id"], 7);
        assert!(r["err"].is_string());
        c.send(r#"{"id":8,"cmd":"ping"}"#);
        assert_eq!(c.reply()["id"], 8);
        c.finish();
    }

    #[test]
    fn reads_an_image_the_app_hands_over() {
        let mut c = Client::start();
        let file = File::open(fixture("archlike.iso")).unwrap();
        crate::fdpass::send(&c.stream, b"\n", &file).unwrap();
        drop(file);
        // The path is only used for the name: the helper reads the file it
        // was given, not the path.
        c.send(r#"{"id":2,"cmd":"imageInfo","path":"/Users/me/Downloads/arch.iso","fd":true}"#);
        let r = c.reply();
        assert_eq!(r["ok"]["name"], "arch.iso");
        assert_eq!(r["ok"]["copyMode"]["supported"], true);
        assert_eq!(r["ok"]["copyMode"]["label"], "ARCH_202409");
        c.finish();
    }

    #[test]
    fn says_when_no_file_was_handed_over() {
        let mut c = Client::start();
        c.send(r#"{"id":3,"cmd":"imageInfo","path":"x.iso","fd":true}"#);
        assert!(c.reply()["err"].as_str().unwrap().contains("hand over"));
        c.finish();
    }

    #[test]
    fn refuses_operations_on_unknown_disks() {
        let mut c = Client::start();
        c.send(
            r#"{"id":4,"cmd":"execute","confirmed":true,"request":{"type":"eraseDisk","disk":"/dev/does-not-exist","filesystem":"exFat","label":"USB"}}"#,
        );
        let r = c.reply();
        assert_eq!(r["id"], 4);
        assert!(r["err"].as_str().unwrap().contains("not found"), "{r}");

        let image = fixture("archlike.iso");
        c.send(&format!(
            r#"{{"id":5,"cmd":"flash","imagePath":"{image}","diskId":"/dev/does-not-exist","mode":"copy","verify":true}}"#
        ));
        let r = c.reply();
        assert_eq!(r["id"], 5);
        assert!(r["err"].as_str().unwrap().contains("not found"), "{r}");

        // Not stuck in "busy" after the failed write.
        c.send(&format!(
            r#"{{"id":6,"cmd":"flash","imagePath":"{image}","diskId":"/dev/does-not-exist","mode":"raw","verify":false}}"#
        ));
        assert!(!c.reply()["err"].as_str().unwrap().contains("already"));
        c.finish();
    }

    #[test]
    fn cancel_is_always_accepted() {
        let mut c = Client::start();
        c.send(r#"{"id":9,"cmd":"cancelFlash"}"#);
        assert_eq!(c.reply(), json!({ "id": 9, "ok": null }));
        c.finish();
    }

    #[test]
    fn serves_one_client_on_a_private_socket() {
        let dir = std::env::temp_dir().join(format!("moondisk-helper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let socket = dir.join("h.sock");
        let owner = unsafe { libc::getuid() };
        let path = socket.clone();
        let server = std::thread::spawn(move || listen(&path, owner));

        let stream = loop {
            if let Ok(s) = UnixStream::connect(&socket) {
                break s;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let mut lines = BufReader::new(stream.try_clone().unwrap());
        (&stream)
            .write_all(b"{\"id\":1,\"cmd\":\"ping\"}\n")
            .unwrap();
        let mut line = String::new();
        lines.read_line(&mut line).unwrap();
        assert!(line.contains("\"ok\""), "{line}");
        // The socket is gone once the app is connected.
        assert!(!socket.exists());
        stream.shutdown(std::net::Shutdown::Both).unwrap();
        server.join().unwrap().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parses_its_arguments() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(run_from_args(&args(&["--listen"])).is_err());
        assert!(
            run_from_args(&args(&["--listen", "/tmp/x", "--owner", "me"]))
                .unwrap_err()
                .contains("user id")
        );
    }
}
