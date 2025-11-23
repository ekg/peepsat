# PeepSat

Satellite image viewer for GOES satellite data.

## Prerequisites

- Rust (cargo)

## Running the Server

```bash
cargo run --bin server
```

The server will start on `http://localhost:8000`

## Usage

Open your browser to `http://localhost:8000` to view the satellite imagery interface.

The server proxies requests to NOAA's GOES satellite imagery CDN and serves the WebGL-based viewer interface.
