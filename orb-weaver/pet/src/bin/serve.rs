use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    time::Duration,
};
fn handle(mut stream: TcpStream, root: PathBuf) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let mut buf = [0; 8192];
    let n = stream.read(&mut buf)?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let mut words = request.lines().next().unwrap_or("").split_whitespace();
    let method = words.next().unwrap_or("");
    let path = words.next().unwrap_or("").split('?').next().unwrap_or("");
    let file = match path {
        "/" | "/index.html" => Some(("index.html", "text/html; charset=utf-8")),
        "/app.js" => Some(("app.js", "text/javascript; charset=utf-8")),
        "/style.css" => Some(("style.css", "text/css; charset=utf-8")),
        "/orb_weaver_pet.wasm" => Some(("orb_weaver_pet.wasm", "application/wasm")),
        _ => None,
    };
    let (status, mime, body) = if method != "GET" && method != "HEAD" {
        (
            "405 Method Not Allowed",
            "text/plain",
            b"Method not allowed".to_vec(),
        )
    } else if let Some((name, mime)) = file {
        match fs::read(root.join(name)) {
            Ok(body) => ("200 OK", mime, body),
            Err(_) => (
                "404 Not Found",
                "text/plain",
                b"Asset missing. Rebuild the pet.".to_vec(),
            ),
        }
    } else {
        ("404 Not Found", "text/plain", b"Not found".to_vec())
    };
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",body.len())?;
    if method != "HEAD" {
        stream.write_all(&body)?
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let port = args
        .windows(2)
        .find(|a| a[0] == "--port")
        .map(|a| a[1].parse::<u16>())
        .transpose()?
        .unwrap_or(0);
    let root = env::current_exe()?
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("web");
    if !root.join("orb_weaver_pet.wasm").exists() {
        return Err("Missing web assets next to the bin folder".into());
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let url = format!("http://{}/", listener.local_addr()?);
    println!("Orb is awake: {url}\nKeep this window open. Press Control-C to close the habitat.");
    if args.iter().any(|s| s == "--open") {
        #[cfg(target_os = "macos")]
        std::process::Command::new("open").arg(&url).spawn()?;
    }
    for stream in listener.incoming() {
        let root = root.clone();
        match stream {
            Ok(stream) => {
                std::thread::spawn(move || {
                    let _ = handle(stream, root);
                });
            }
            Err(e) => eprintln!("Connection: {e}"),
        }
    }
    Ok(())
}
