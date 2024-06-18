use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::fs::File;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::str;

#[derive(Debug, Clone)]
struct ServerConfig {
    servername: String,
    port: u16,
    proxy_pass: String,
}

fn parse_config() -> Vec<ServerConfig> {
    let mut file = File::open("config.conf").expect("Config file not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Failed to read config file");

    let mut configs = Vec::new();
    let mut current_config = ServerConfig {
        servername: String::new(),
        port: 80,
        proxy_pass: String::new(),
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with("servername") {
            current_config.servername = line.split_whitespace().nth(1).unwrap().replace(";", "");
        } else if line.starts_with("port") {
            current_config.port = line.split_whitespace().nth(1).unwrap().replace(";", "").parse().unwrap();
        } else if line.starts_with("proxy_pass") {
            current_config.proxy_pass = line.split_whitespace().nth(1).unwrap().replace(";", "");
        } else if line == "}" {
            configs.push(current_config.clone());
        }
    }

    configs
}

fn handle_client(mut stream: TcpStream, config: Arc<HashMap<String, ServerConfig>>) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();
    let request = str::from_utf8(&buffer).unwrap();

    let host_line = request.lines().find(|line| line.starts_with("Host:"));
    let host = if let Some(host_line) = host_line {
        host_line.split_whitespace().nth(1).unwrap()
    } else {
        ""
    };

    let response = if host.is_empty() || request.lines().next().unwrap().contains("/") {
        // トップ画面を返す
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><head><title>MiHTTP</title></head><body><h1>Welcome to MiHTTP</h1><p>Your reverse proxy server is running!</p></body></html>".to_string()
    } else if let Some(server_config) = config.get(host) {
        let uri = format!("{}{}", server_config.proxy_pass, request.lines().next().unwrap().split_whitespace().nth(1).unwrap());
        let response = forward_request(&uri);
        response
    } else {
        "HTTP/1.1 404 Not Found\r\nContent-Type: text/plain\r\n\r\nNot Found".to_string()
    };

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn forward_request(uri: &str) -> String {
    let mut parts = uri.split('/');
    let host = parts.next().unwrap();
    let path = parts.collect::<Vec<&str>>().join("/");

    let mut stream = TcpStream::connect(host).unwrap();
    let request = format!("GET /{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host);
    stream.write(request.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut buffer = Vec::new();
    stream.read_to_end(&mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

fn main() {
    let configs = parse_config();
    let mut config_map = HashMap::new();
    for config in configs {
        config_map.insert(config.servername.clone(), config);
    }
    let config = Arc::new(config_map);

    let listener = TcpListener::bind("0.0.0.0:80").unwrap();
    println!("Listening on port 80");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    handle_client(stream, config);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}
