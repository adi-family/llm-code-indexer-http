# ADI HTTP

HTTP API server for ADI (AI-powered Developer Intelligence) - exposing code indexing and search via REST endpoints.

## Overview

`adi-http` provides a REST API interface for code indexing and semantic search operations. Built with Axum for high-performance async HTTP handling.

## Installation

```bash
cargo build --release
# Binary available at: target/release/adi-http
```

## API Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/index` | Index a codebase |
| GET | `/search` | Semantic code search |
| GET | `/symbols` | List indexed symbols |
| GET | `/files` | List indexed files |
| GET | `/status` | Get indexing status |
| GET | `/health` | Health check |

## Quick Start

```bash
# Start the server
adi-http --port 8080

# Search via API
curl "http://localhost:8080/search?q=authentication"
```

## Configuration

Server configuration via environment variables or command-line arguments:

- `ADI_HTTP_PORT` - Server port (default: 8080)
- `ADI_HTTP_HOST` - Bind address (default: 127.0.0.1)

## License

BSL-1.1 - See [LICENSE](LICENSE) for details.
