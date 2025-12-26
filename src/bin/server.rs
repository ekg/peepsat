use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashMap;
use std::time::SystemTime;
use tiny_http::{Server, Response, Request, Header};

const SLIDER_BASE_URL: &str = "https://rammb-slider.cira.colostate.edu";
const CACHE_MAX_SIZE: u64 = 500 * 1024 * 1024; // 500 MB cache limit

// LRU cache tracking
struct CacheEntry {
    path: PathBuf,
    size: u64,
    last_access: SystemTime,
}

lazy_static::lazy_static! {
    static ref CACHE_DIR: PathBuf = {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let cache_dir = PathBuf::from(home).join(".peepsat").join("tiles");
        fs::create_dir_all(&cache_dir).ok();
        cache_dir
    };
    static ref CACHE_INDEX: Mutex<HashMap<String, CacheEntry>> = Mutex::new(HashMap::new());
    // HTTP client that follows redirects
    static ref HTTP_CLIENT: reqwest::blocking::Client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();
}

fn cache_key(sat: &str, timestamp: &str, zoom: u32, x: u32, y: u32) -> String {
    format!("{}_{}_{}_{}_{}", sat, timestamp, zoom, x, y)
}

fn cache_path(key: &str) -> PathBuf {
    CACHE_DIR.join(format!("{}.png", key))
}

fn get_cached_tile(key: &str) -> Option<Vec<u8>> {
    let path = cache_path(key);
    if path.exists() {
        if let Ok(data) = fs::read(&path) {
            // Update last access time in index
            if let Ok(mut index) = CACHE_INDEX.lock() {
                if let Some(entry) = index.get_mut(key) {
                    entry.last_access = SystemTime::now();
                }
            }
            return Some(data);
        }
    }
    None
}

fn put_cached_tile(key: &str, data: &[u8]) {
    let path = cache_path(key);
    if fs::write(&path, data).is_ok() {
        let size = data.len() as u64;
        if let Ok(mut index) = CACHE_INDEX.lock() {
            index.insert(key.to_string(), CacheEntry {
                path: path.clone(),
                size,
                last_access: SystemTime::now(),
            });

            // Check if we need to evict old entries
            let total_size: u64 = index.values().map(|e| e.size).sum();
            if total_size > CACHE_MAX_SIZE {
                evict_lru(&mut index, total_size - CACHE_MAX_SIZE);
            }
        }
    }
}

fn evict_lru(index: &mut HashMap<String, CacheEntry>, bytes_to_free: u64) {
    let mut entries: Vec<_> = index.iter().collect();
    entries.sort_by_key(|(_, e)| e.last_access);

    let mut freed = 0u64;
    let mut to_remove = Vec::new();

    for (key, entry) in entries {
        if freed >= bytes_to_free {
            break;
        }
        if fs::remove_file(&entry.path).is_ok() {
            freed += entry.size;
            to_remove.push(key.clone());
        }
    }

    for key in to_remove {
        index.remove(&key);
        println!("Cache evicted: {}", key);
    }
    println!("Cache freed {} bytes", freed);
}

fn init_cache_index() {
    // Scan cache directory and rebuild index on startup
    if let Ok(entries) = fs::read_dir(&*CACHE_DIR) {
        if let Ok(mut index) = CACHE_INDEX.lock() {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let path = entry.path();
                        if let Some(stem) = path.file_stem() {
                            let key = stem.to_string_lossy().to_string();
                            index.insert(key, CacheEntry {
                                path,
                                size: meta.len(),
                                last_access: meta.modified().unwrap_or(SystemTime::now()),
                            });
                        }
                    }
                }
            }
            let total: u64 = index.values().map(|e| e.size).sum();
            println!("Cache initialized: {} entries, {:.1} MB", index.len(), total as f64 / 1024.0 / 1024.0);
        }
    }
}

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

fn get_query_param<'a>(url: &'a str, name: &str) -> Option<String> {
    url.find('?')
        .map(|pos| &url[pos+1..])
        .and_then(|query| {
            query.split('&')
                .find(|s| s.starts_with(&format!("{}=", name)))
                .and_then(|s| s.strip_prefix(&format!("{}=", name)))
                .map(|s| urlencoding::decode(s).unwrap_or_default().into_owned())
        })
}

fn get_cdn_url(url: &str) -> String {
    get_query_param(url, "cdn").unwrap_or_else(|| SLIDER_BASE_URL.to_string())
}

fn handle_slider_latest(request: Request) {
    let url = request.url();
    let sat = get_query_param(url, "sat").unwrap_or_else(|| "19".to_string());
    let cdn = get_cdn_url(url);

    let target = format!(
        "{}/data/json/{}/full_disk/geocolor/latest_times.json",
        cdn, satellite_id(&sat)
    );

    println!("Fetching latest times: {}", target);
    match HTTP_CLIENT.get(&target).send() {
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
    let sat = get_query_param(url, "sat").unwrap_or_else(|| "19".to_string());
    let cdn = get_cdn_url(url);

    let target = format!(
        "{}/data/json/{}/full_disk/geocolor/available_dates.json",
        cdn, satellite_id(&sat)
    );

    println!("Fetching available dates: {}", target);
    match HTTP_CLIENT.get(&target).send() {
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
    // Parse: /slider-tile?sat=19&t=20231026153000&x=7&y=8&z=4&cdn=...
    let url = request.url();
    let sat = get_query_param(url, "sat").unwrap_or_else(|| "19".to_string());
    let timestamp = get_query_param(url, "t").unwrap_or_else(|| "0".to_string());
    let x: u32 = get_query_param(url, "x").and_then(|s| s.parse().ok()).unwrap_or(0);
    let y: u32 = get_query_param(url, "y").and_then(|s| s.parse().ok()).unwrap_or(0);
    let date = get_query_param(url, "d").unwrap_or_default(); // YYYYMMDD format
    let zoom: u32 = get_query_param(url, "z").and_then(|s| s.parse().ok()).unwrap_or(4);
    let cdn = get_cdn_url(url);

    // Clamp zoom to valid range (0-4 for GOES, 0-3 for Meteosat)
    let max_zoom = satellite_max_zoom(&sat);
    let zoom = zoom.min(max_zoom);

    // Check cache first
    let key = cache_key(&sat, &timestamp, zoom, x, y);
    if let Some(data) = get_cached_tile(&key) {
        println!("Cache hit: ({}, {}) z{}", x, y, zoom);
        let response = Response::from_data(data)
            .with_header(Header::from_bytes("Content-Type", "image/png").unwrap())
            .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
            .with_header(Header::from_bytes("X-Cache", "HIT").unwrap());
        let _ = request.respond(response);
        return;
    }

    // Parse date into year/month/day
    let (year, month, day) = if date.len() == 8 {
        let y: u32 = date[0..4].parse().unwrap_or(2024);
        let m: u32 = date[4..6].parse().unwrap_or(1);
        let d: u32 = date[6..8].parse().unwrap_or(1);
        (y, m, d)
    } else {
        (2024, 1, 1)
    };

    // URL format from satpaper: {base}/data/imagery/{year}/{month}/{day}/{sat_id}---full_disk/geocolor/{timestamp}/{zoom}/{x:03}_{y:03}.png
    let target = format!(
        "{}/data/imagery/{:04}/{:02}/{:02}/{}---full_disk/geocolor/{}/{:02}/{:03}_{:03}.png",
        cdn, year, month, day, satellite_id(&sat), timestamp, zoom, x, y
    );

    println!("Fetching tile ({}, {}) z{}: {}", x, y, zoom, target);
    match HTTP_CLIENT.get(&target).send() {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().unwrap_or_default();
            println!("Tile ({}, {}) status={} len={}", x, y, status, bytes.len());

            if status.is_success() && !bytes.is_empty() {
                // Cache the tile
                put_cached_tile(&key, &bytes);

                let response = Response::from_data(bytes.to_vec())
                    .with_header(Header::from_bytes("Content-Type", "image/png").unwrap())
                    .with_header(Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap())
                    .with_header(Header::from_bytes("X-Cache", "MISS").unwrap());
                let _ = request.respond(response);
            } else {
                let _ = request.respond(Response::from_data(bytes.to_vec()).with_status_code(status.as_u16()));
            }
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
    let resp = HTTP_CLIENT.get(&target).send();
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
    init_cache_index();

    let server = Server::http("0.0.0.0:8000").unwrap();
    println!("Server running on http://0.0.0.0:8000");
    println!("Cache directory: {:?}", *CACHE_DIR);

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