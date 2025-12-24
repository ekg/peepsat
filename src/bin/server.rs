use std::fs;
use tiny_http::{Server, Response, Request, Header};

const SLIDER_BASE_URL: &str = "https://rammb-slider.cira.colostate.edu";

// Satellite configurations matching satpaper
fn satellite_id(sat: &str) -> &'static str {
    match sat {
        "18" => "goes-18",
        "19" => "goes-19",
        "himawari" => "himawari",
        "meteosat9" => "meteosat-9",
        "meteosat10" => "meteosat-0deg",
        _ => "goes-19",
    }
}

fn satellite_max_zoom(sat: &str) -> u32 {
    match sat {
        "meteosat9" | "meteosat10" => 3,
        _ => 4,
    }
}

fn handle_slider_latest(request: Request) {
    let url = request.url();
    let sat = if let Some(pos) = url.find('?') {
        url[pos+1..].split('&')
            .find(|s| s.starts_with("sat="))
            .and_then(|s| s.strip_prefix("sat="))
            .unwrap_or("19")
    } else {
        "19"
    };

    let target = format!(
        "{}/data/json/{}/full_disk/geocolor/latest_times.json",
        SLIDER_BASE_URL, satellite_id(sat)
    );

    println!("Fetching latest times: {}", target);
    match reqwest::blocking::get(&target) {
        Ok(r) => {
            let bytes = r.bytes().unwrap_or_default();
            let response = Response::from_data(bytes.to_vec())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap());
            let _ = request.respond(response);
        }
        Err(e) => {
            println!("Slider latest error: {:?}", e);
            let _ = request.respond(Response::from_string("Failed").with_status_code(502));
        }
    }
}

fn handle_slider_dates(request: Request) {
    let url = request.url();
    let sat = if let Some(pos) = url.find('?') {
        url[pos+1..].split('&')
            .find(|s| s.starts_with("sat="))
            .and_then(|s| s.strip_prefix("sat="))
            .unwrap_or("19")
    } else {
        "19"
    };

    let target = format!(
        "{}/data/json/{}/full_disk/geocolor/available_dates.json",
        SLIDER_BASE_URL, satellite_id(sat)
    );

    println!("Fetching available dates: {}", target);
    match reqwest::blocking::get(&target) {
        Ok(r) => {
            let bytes = r.bytes().unwrap_or_default();
            let response = Response::from_data(bytes.to_vec())
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap());
            let _ = request.respond(response);
        }
        Err(e) => {
            println!("Slider dates error: {:?}", e);
            let _ = request.respond(Response::from_string("Failed").with_status_code(502));
        }
    }
}

fn handle_slider_tile(request: Request) {
    // Parse: /slider-tile?sat=19&t=20231026153000&x=7&y=8&z=4
    let url = request.url();
    let query = url.find('?').map(|p| &url[p+1..]).unwrap_or("");

    let get_param = |name: &str| -> Option<&str> {
        query.split('&')
            .find(|s| s.starts_with(&format!("{}=", name)))
            .and_then(|s| s.strip_prefix(&format!("{}=", name)))
    };

    let sat = get_param("sat").unwrap_or("19");
    let timestamp = get_param("t").unwrap_or("0");
    let x: u32 = get_param("x").and_then(|s| s.parse().ok()).unwrap_or(0);
    let y: u32 = get_param("y").and_then(|s| s.parse().ok()).unwrap_or(0);
    let date = get_param("d").unwrap_or(""); // YYYYMMDD format
    let zoom: u32 = get_param("z").and_then(|s| s.parse().ok()).unwrap_or(4); // Default to max zoom

    // Parse date into year/month/day
    let (year, month, day) = if date.len() == 8 {
        let y: u32 = date[0..4].parse().unwrap_or(2024);
        let m: u32 = date[4..6].parse().unwrap_or(1);
        let d: u32 = date[6..8].parse().unwrap_or(1);
        (y, m, d)
    } else {
        (2024, 1, 1)
    };

    // Clamp zoom to valid range (0-4 for GOES, 0-3 for Meteosat)
    let max_zoom = satellite_max_zoom(sat);
    let zoom = zoom.min(max_zoom);

    // URL format from satpaper: {base}/data/imagery/{year}/{month}/{day}/{sat_id}---full_disk/geocolor/{timestamp}/{zoom}/{x:03}_{y:03}.png
    let target = format!(
        "{}/data/imagery/{:04}/{:02}/{:02}/{}---full_disk/geocolor/{}/{:02}/{:03}_{:03}.png",
        SLIDER_BASE_URL, year, month, day, satellite_id(sat), timestamp, zoom, x, y
    );

    println!("Fetching tile ({}, {}): {}", x, y, target);
    match reqwest::blocking::get(&target) {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().unwrap_or_default();
            println!("Tile ({}, {}) status={} len={}", x, y, status, bytes.len());
            let mut response = Response::from_data(bytes.to_vec());
            if status.is_success() {
                response = response
                    .with_header(Header::from_bytes("Content-Type", "image/png").unwrap())
                    .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap());
            }
            let _ = request.respond(response);
        }
        Err(e) => {
            println!("Tile error: {:?}", e);
            let _ = request.respond(Response::from_string("Failed").with_status_code(502));
        }
    }
}

fn handle_goes_proxy(request: Request) {
    // Parse query string for timestamp, satellite, and resolution parameters
    let url = request.url();
    let (timestamp, satellite, resolution) = if let Some(pos) = url.find('?') {
        let query = &url[pos+1..];
        let ts = query.split('&')
            .find(|s| s.starts_with("t="))
            .and_then(|s| s.strip_prefix("t="));
        let sat = query.split('&')
            .find(|s| s.starts_with("sat="))
            .and_then(|s| s.strip_prefix("sat="))
            .unwrap_or("18");
        let res = query.split('&')
            .find(|s| s.starts_with("res="))
            .and_then(|s| s.strip_prefix("res="))
            .unwrap_or("5424x5424");
        (ts, sat, res)
    } else {
        (None, "18", "5424x5424")
    };

    let target = if let Some(ts) = timestamp {
        // Format: YYYYDDDHHMM -> https://cdn.star.nesdis.noaa.gov/GOES{sat}/ABI/FD/GEOCOLOR/YYYYDDDHHMM_GOES{sat}-ABI-FD-GEOCOLOR-{res}.jpg
        format!("https://cdn.star.nesdis.noaa.gov/GOES{}/ABI/FD/GEOCOLOR/{}_GOES{}-ABI-FD-GEOCOLOR-{}.jpg", satellite, ts, satellite, resolution)
    } else {
        format!("https://cdn.star.nesdis.noaa.gov/GOES{}/ABI/FD/GEOCOLOR/latest.jpg", satellite)
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
        if url.starts_with("/slider-latest") {
            handle_slider_latest(request);
            continue;
        }
        if url.starts_with("/slider-dates") {
            handle_slider_dates(request);
            continue;
        }
        if url.starts_with("/slider-tile") {
            handle_slider_tile(request);
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