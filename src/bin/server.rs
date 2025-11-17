use std::fs;
use tiny_http::{Server, Response, Request, Header};

fn handle_goes_proxy(request: Request) {
    // Parse query string for timestamp parameter
    let url = request.url();
    let timestamp = if let Some(pos) = url.find('?') {
        let query = &url[pos+1..];
        query.split('&')
            .find(|s| s.starts_with("t="))
            .and_then(|s| s.strip_prefix("t="))
    } else {
        None
    };

    let target = if let Some(ts) = timestamp {
        format!("https://cdn.star.nesdis.noaa.gov/GOES16/ABI/FD/GEOCOLOR/{}_GOES16-ABI-FD-GEOCOLOR-10848x10848.jpg", ts)
    } else {
        "https://cdn.star.nesdis.noaa.gov/GOES16/ABI/FD/GEOCOLOR/latest.jpg".to_string()
    };

    println!("Fetching: {}", target);
    let resp = reqwest::blocking::get(&target);
    match resp {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().unwrap_or_default();
            println!("GOES proxy success: status={} len={}", status, bytes.len());
            let mut response = Response::from_data(bytes.to_vec());
            if status.is_success() {
                response = response.with_header(Header::from_bytes("Content-Type", "image/jpeg").unwrap());
            }
            let _ = request.respond(response);
        }
        Err(e) => {
            println!("GOES proxy error: {:?}", e);
            let _ = request.respond(Response::from_string("Failed to fetch GOES image").with_status_code(502));
        }
    }
}


fn main() {
    let server = Server::http("0.0.0.0:8000").unwrap();
    println!("Server running on http://0.0.0.0:8000");

    for request in server.incoming_requests() {
        let url = request.url();
        if url.starts_with("/goes-proxy") {
            handle_goes_proxy(request);
            continue;
        }

        let path = if url == "/" || url.starts_with("/?") {
            "index.html"
        } else {
            &url[1..]
        };

        let content_type = if path.ends_with(".html") {
            "text/html"
        } else if path.ends_with(".js") {
            "application/javascript"
        } else if path.ends_with(".wasm") {
            "application/wasm"
        } else {
            "text/plain"
        };

        match fs::read(path) {
            Ok(data) => {
                let response = Response::from_data(data).with_header(
                    tiny_http::Header::from_bytes("Content-Type", content_type).unwrap()
                );
                request.respond(response).unwrap();
            }
            Err(_) => {
                request.respond(Response::from_string("404 Not Found").with_status_code(404)).unwrap();
            }
        }
    }
}