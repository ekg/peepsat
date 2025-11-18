use std::fs;
use std::io::Cursor;
use tiny_http::{Server, Response, Request, Header};
use image::{ImageOutputFormat, codecs::tiff::TiffDecoder};

fn convert_tiff_to_jpeg(tiff_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Decode TIFF
    let decoder = TiffDecoder::new(Cursor::new(tiff_bytes))?;
    let img = image::DynamicImage::from_decoder(decoder)?;

    // Encode as JPEG
    let mut jpeg_bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut jpeg_bytes), ImageOutputFormat::Jpeg(85))?;

    Ok(jpeg_bytes)
}

fn handle_goes_proxy(request: Request) {
    // Parse query string for timestamp parameter (YYYY-MM-DD-HHMM format)
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
        // Format: YYYY-MM-DD-HHMM -> https://mesonet.agron.iastate.edu/archive/data/YYYY/MM/DD/GIS/sat/conus_goes_ir4km_HHMM.tif
        let parts: Vec<&str> = ts.split('-').collect();
        if parts.len() == 4 {
            // parts[0]=YYYY, parts[1]=MM, parts[2]=DD, parts[3]=HHMM
            format!("https://mesonet.agron.iastate.edu/archive/data/{}/{}/{}/GIS/sat/conus_goes_ir4km_{}.tif",
                parts[0], parts[1], parts[2], parts[3])
        } else {
            "https://cdn.star.nesdis.noaa.gov/GOES18/ABI/FD/GEOCOLOR/latest.jpg".to_string()
        }
    } else {
        "https://cdn.star.nesdis.noaa.gov/GOES18/ABI/FD/GEOCOLOR/latest.jpg".to_string()
    };

    println!("Fetching: {}", target);
    let resp = reqwest::blocking::get(&target);
    match resp {
        Ok(r) => {
            let status = r.status();
            let bytes = r.bytes().unwrap_or_default();
            println!("GOES proxy success: status={} len={}", status, bytes.len());

            // Convert TIFF to JPEG for browser compatibility
            let response_bytes = if target.ends_with(".tif") && status.is_success() {
                match convert_tiff_to_jpeg(&bytes) {
                    Ok(jpeg_bytes) => {
                        println!("Converted TIFF to JPEG: {} -> {} bytes", bytes.len(), jpeg_bytes.len());
                        jpeg_bytes
                    }
                    Err(e) => {
                        println!("TIFF conversion failed: {:?}, returning original", e);
                        bytes.to_vec()
                    }
                }
            } else {
                bytes.to_vec()
            };

            let mut response = Response::from_data(response_bytes);
            if status.is_success() {
                // Always serve as JPEG after conversion
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