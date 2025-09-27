# wapp_simple_stats_rust — Simple visitor counter with SQLite, Rocket, Tera

Lightweight web counter written in Rust. It stores hits in SQLite and exposes:
- a badge endpoint you can embed anywhere to increment and display a named counter
- a protected statistics dashboard with charts and tables

UI is rendered with Tera templates and styled via Pico.css (CDN). Charts are rendered with Chart.js (CDN). Supports automatic light/dark theme selection.

## Features

- Named counters via `/counter/<path>` (returns an SVG badge)
- Statistics dashboard:
  - Index page: grouped counts by path
  - Path page: daily counts for last 30 entries + last 10 raw visits
- Basic Auth (configurable)
- SQLite storage (file on disk)
- Auto light/dark theme, responsive layout

## Endpoints

- `GET /counter/<path>`
  - Increments counter for `<path>` and returns a small SVG badge
  - Example: `<img src="/counter/example" alt="statistics badge" />`
- `GET /statistics`
  - Protected by Basic Auth (if enabled)
  - Summary table of all paths
- `GET /statistics/__all__`
  - Protected by Basic Auth (if enabled)
  - Daily counts (last 30 rows) aggregated for all paths + last 10 recent visits
- `GET /statistics/<path>`
  - Protected by Basic Auth (if enabled)
  - Daily counts (last 30 rows) for specific path + last 10 recent visits
- `GET /statistics_self_full_json`
  - Full dataset as JSON (not protected by default)
  - Useful for tooling/integrations

## UI/UX

- Templates live under `templates/`
  - Base layout: `templates/base.tera`
  - Landing page: `templates/landing.tera`
  - Statistics index: `templates/statistics/index.tera`
  - Statistics path: `templates/statistics/path.tera`
- Uses Pico.css (CDN) and Chart.js (CDN)
- Auto light/dark theme through the browser preferences

## Configuration

Configuration is loaded from `config.yaml` at startup. An example file is provided:
- `config.example.yaml`

Create your own config:
```bash
cp config.example.yaml config.yaml
```

`config.yaml` structure:
```yaml
site:
  title: "Simple Stats"
theme:
  auto: true    # auto-select light/dark
auth:
  enabled: true # protect /statistics pages with Basic Auth
  basic:
    username: "admin"
    password: "password"
```

Notes:
- `config.yaml` is ignored by git by default.
- When `auth.enabled` is true, routes `/statistics` and `/statistics/<path>` require HTTP Basic credentials.
- `site.title` is shown in the header and page titles.

## Requirements

- Rust toolchain (stable)
- SQLite (libsqlite3 via `rusqlite`)

## Build and Run

Development build:
```bash
cargo build
```

Run:
```bash
cargo run
```

By default Rocket binds to `localhost:8000` (or as configured by Rocket). Open:
- `http://localhost:8000/` for landing page
- `http://localhost:8000/statistics` for dashboard (enter Basic credentials when prompted)

If you have Basic Auth enabled and you want to access the dashboard via curl:
```bash
curl -u admin:password http://localhost:8000/statistics
```

## Usage Examples

Embed a counter badge on any site:
```html
<img src="https://your-host/counter/my-page" alt="statistics" />
```

Open charts for a specific path:
- `https://your-host/statistics/my-page`

View all daily counts:
- `https://your-host/statistics/__all__`

Fetch all data as JSON:
- `https://your-host/statistics_self_full_json`

## Database

SQLite file path is configured in code as:
- `./wapp_simple_stats_rust.db`

Table schema (created automatically on launch if missing):
```sql
CREATE TABLE IF NOT EXISTS visitors (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path VARCHAR(255) NOT NULL,
  timestamp DATE DEFAULT (datetime('now','localtime')),
  ip VARCHAR(50) NOT NULL,
  json VARCHAR(4000) NOT NULL
);
```

## Screenshots

Historical screenshot:
- `images/2023-02-05_00-28.png`

The new UI uses Tera + Pico.css + Chart.js and renders:
- A clean dashboard for all paths
- Path-level chart of daily counts and a “Last 10” table

## Notes and Caveats

- The JSON column stores request headers as a JSON string for debugging/analysis.
- Badge endpoint returns an SVG; it is safe to embed as an `<img>` source in static sites.
- `/statistics_self_full_json` is unauthenticated by default; protect it if needed.

## License

MIT (or the same license you prefer for this repository).
