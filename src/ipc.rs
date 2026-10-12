use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR")
        && !runtime_dir.trim().is_empty()
    {
        return PathBuf::from(runtime_dir).join("madlion-gamepad.sock");
    }
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("madlion-gamepad-{uid}.sock"))
}

pub fn send_ipc_command(cmd: &str) -> io::Result<String> {
    let path = socket_path();
    let mut stream = UnixStream::connect(&path)?;
    stream.write_all(cmd.as_bytes())?;
    if !cmd.ends_with('\n') {
        stream.write_all(b"\n")?;
    }
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    Ok(response.trim().to_string())
}

pub struct IpcServer {
    is_running: Arc<AtomicBool>,
    socket_path: PathBuf,
    thread_handle: Option<JoinHandle<()>>,
}

impl IpcServer {
    pub fn start<F>(handler: F) -> io::Result<Self>
    where
        F: Fn(&str) -> String + Send + Sync + 'static,
    {
        let path = socket_path();
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        let listener = UnixListener::bind(&path)?;
        listener.set_nonblocking(true)?;

        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();
        let path_clone = path.clone();
        let handler_arc = Arc::new(handler);

        let thread_handle = thread::spawn(move || {
            while is_running_clone.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let h = handler_arc.clone();
                        thread::spawn(move || {
                            handle_client(stream, h);
                        });
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(25));
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            let _ = fs::remove_file(&path_clone);
        });

        Ok(Self {
            is_running,
            socket_path: path,
            thread_handle: Some(thread_handle),
        })
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread_handle.take() {
            let _ = h.join();
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn handle_client<F>(mut stream: UnixStream, handler: Arc<F>)
where
    F: Fn(&str) -> String,
{
    let mut reader = BufReader::new(&stream);

    let mut line = String::new();
    if reader.read_line(&mut line).is_ok() {
        let trimmed = line.trim();
        let response = handler(trimmed);
        let _ = stream.write_all(response.as_bytes());
        if !response.ends_with('\n') {
            let _ = stream.write_all(b"\n");
        }
        let _ = stream.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_communication_roundtrip() {
        let server = IpcServer::start(|cmd| match cmd {
            "toggle-override" => "ok: override toggled".to_string(),
            "status" => "ok: status active".to_string(),
            _ => "error: unknown command".to_string(),
        })
        .unwrap();

        // send toggle-override
        let resp = send_ipc_command("toggle-override").unwrap();
        assert_eq!(resp, "ok: override toggled");

        // send status
        let resp_status = send_ipc_command("status").unwrap();
        assert_eq!(resp_status, "ok: status active");

        // send invalid command
        let resp_err = send_ipc_command("unknown").unwrap();
        assert_eq!(resp_err, "error: unknown command");

        drop(server);
    }
}
