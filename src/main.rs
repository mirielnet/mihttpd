use hyper::{Body, Client, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::client::HttpConnector;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::sync::Arc;
use std::collections::HashMap;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::multispace0;
use nom::sequence::{delimited, pair};
use nom::combinator::map;
use nom::multi::many0;
use nom::IResult;

#[derive(Debug, Clone)]
struct ServerConfig {
    servername: String,
    port: u16,
    proxy_pass: String,
}

fn parse_word(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '.' || c == ':')(input)
}

fn parse_server(input: &str) -> IResult<&str, ServerConfig> {
    let (input, _) = tag("server")(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("{")(input)?;
    let (input, _) = multispace0(input)?;

    let (input, servername) = map(
        delimited(pair(tag("servername"), multispace0), parse_word, tag(";")),
        |s: &str| s.to_string(),
    )(input)?;
    let (input, _) = multispace0(input)?;

    let (input, port) = map(
        delimited(pair(tag("port"), multispace0), parse_word, tag(";")),
        |s: &str| s.parse::<u16>().unwrap(),
    )(input)?;
    let (input, _) = multispace0(input)?;

    let (input, proxy_pass) = map(
        delimited(pair(tag("proxy_pass"), multispace0), parse_word, tag(";")),
        |s: &str| s.to_string(),
    )(input)?;
    let (input, _) = multispace0(input)?;

    let (input, _) = tag("}")(input)?;

    Ok((input, ServerConfig {
        servername,
        port,
        proxy_pass,
    }))
}

fn parse_config(input: &str) -> IResult<&str, Vec<ServerConfig>> {
    many0(delimited(multispace0, parse_server, multispace0))(input)
}

async fn load_config() -> Vec<ServerConfig> {
    let mut file = File::open("config.conf").await.expect("Config file not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents).await.expect("Failed to read config file");

    match parse_config(&contents) {
        Ok((_, config)) => config,
        Err(err) => panic!("Failed to parse config file: {:?}", err),
    }
}

async fn proxy(
    req: Request<Body>,
    client: Arc<Client<HttpConnector>>,
    config: Arc<HashMap<String, ServerConfig>>,
) -> Result<Response<Body>, hyper::Error> {
    let host = req.headers().get("host").and_then(|h| h.to_str().ok()).unwrap_or("");
    if host.is_empty() || req.uri().path() == "/" {
        // トップ画面を返す
        Ok(Response::new(Body::from("<html><head><title>MiHTTP</title></head><body><h1>Welcome to MiHTTP</h1><p>Your reverse proxy server is running!</p></body></html>")))
    } else {
        if let Some(server_config) = config.get(host) {
            let uri = format!("{}{}", server_config.proxy_pass, req.uri().path_and_query().map(|x| x.as_str()).unwrap_or(""));
            let proxied_request = Request::builder()
                .method(req.method())
                .uri(uri)
                .body(req.into_body())
                .unwrap();
            client.request(proxied_request).await
        } else {
            Ok(Response::builder().status(404).body(Body::from("Not Found")).unwrap())
        }
    }
}

#[tokio::main]
async fn main() {
    let configs = load_config().await;
    let mut config_map = HashMap::new();
    for config in configs {
        config_map.insert(config.servername.clone(), config);
    }
    let config = Arc::new(config_map);
    let client = Arc::new(Client::new());

    let make_svc = make_service_fn(|_conn| {
        let client = Arc::clone(&client);
        let config = Arc::clone(&config);
        async {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                proxy(req, Arc::clone(&client), Arc::clone(&config))
            }))
        }
    });

    let addr = ([0, 0, 0, 0], 80).into();
    let server = Server::bind(&addr).serve(make_svc);

    println!("Listening on http://{}", addr);
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}
