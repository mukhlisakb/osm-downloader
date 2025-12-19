OSM Downloader
==============

OSM Downloader is a small terminal application for downloading OpenStreetMap
extracts from the Geofabrik servers and exploring the data with DuckDB.

The UI is based on `ratatui` and runs entirely in your terminal. Downloaded data
is stored in a local DuckDB database with the DuckDB `spatial` extension
enabled, so you can run SQL queries over the imported OSM tables.


Prerequisites
-------------

- A recent Rust toolchain (stable) installed via `rustup`
- A POSIX-compatible shell (macOS or most Linux distributions)

You do **not** need to install DuckDB separately. The project uses the bundled
DuckDB library via the `duckdb` Rust crate (`features = ["bundled"]` in
`Cargo.toml`), so the database engine is compiled into the binary.


Quick Start
-----------

From the project root:

1. Build the binary:

   ```bash
   cargo build --release
   ```

2. Run the TUI:

   ```bash
   ./target/release/osm-downloader
   ```

Alternatively, you can use the helper script created in `bin/osm-downloader.sh`:

```bash
./bin/osm-downloader.sh
```

The script checks for `cargo`, builds the project in release mode if needed, and
then starts the TUI.


Data Directories
----------------

On startup, the application creates a per-user data directory using the
`directories` crate:

- macOS: `~/Library/Application Support/com.osm-downloader/osm-downloader`
- Linux: `~/.local/share/com.osm-downloader/osm-downloader` (depending on XDG)

Inside this directory the app creates:

- `osm.duckdb` – the DuckDB database file
- `downloads/` – downloaded `.osm.pbf` or `.shp.zip` archives
- `logs/osm-downloader.log` – rotating log files written by `tracing`


Running the Application
-----------------------

When you start the application you will see two tabs:

- `Download`
- `Database / Query`

Use the `Download` tab to fetch data and the `Database / Query` tab to run SQL
against the imported tables.


Download Tab
------------

Fields:

- `Continent (e.g. Asia)` – required
- `Country (e.g. Indonesia)` – required
- `Region (Optional, e.g. Kalimantan)` – optional subregion
- `Format (Press Space to Toggle)` – choose:
  - `OSM PBF (.osm.pbf)`
  - `Shapefile (.shp.zip)`

Keyboard controls:

- `Tab` – move focus between fields
- `Enter` – start download using the current continent/country/region/format
- `Space` – toggle between PBF and Shapefile when the Format field is focused
- `Ctrl+b` – switch between `Download` and `Database / Query` tabs
- `q` – quit the application (when focus is not inside a text input)

During a download the progress bar at the bottom of the tab shows:

- Percentage complete
- Status text (e.g. “Downloading…”, “Saved to …”)

When a download reaches 100%, the app:

1. Writes the file into the user data `downloads/` directory
2. Records the download metadata into the `downloads` table in DuckDB
3. Automatically imports the file into DuckDB as a table named `imported_data`
4. Sends a log message such as “Import successful.” or “Import failed: …”


Database / Query Tab
--------------------

The `Database / Query` tab lets you run arbitrary SQL queries against the local
DuckDB database.

Default behaviour:

- After a successful import, the SQL editor is pre-filled with
  `SELECT * FROM imported_data LIMIT 10;`
- The query is executed automatically once, so you immediately see a preview of
  the imported data.

Keyboard controls in this tab:

- `Ctrl+e` – execute the current query
- `Ctrl+Enter` – execute the current query
- `Ctrl+Shift+Enter` – execute the current query
- `F5` – execute the current query

The result panel shows a fixed-width, boxy table:

- Column headers with padding
- A separator line
- Up to 50 rows of data
- If more rows are available, the final line shows
  `... (more rows truncated)`

Long text values are truncated to a reasonable width with an ellipsis so the
table stays readable in the terminal.


DuckDB Schema
-------------

On first run the application:

1. Creates the DuckDB database `osm.duckdb` in the per-user data directory
2. Attempts to install and load the `spatial` extension:

   ```sql
   INSTALL spatial;
   LOAD spatial;
   ```

3. Creates a metadata table for downloads:

   ```sql
   CREATE TABLE IF NOT EXISTS downloads (
       id           INTEGER PRIMARY KEY,
       url          VARCHAR,
       local_path   VARCHAR,
       downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
   );
   ```

OSM imports:

- For `.osm.pbf` files the app runs:

  ```sql
  CREATE TABLE imported_data AS
  SELECT * FROM ST_ReadOSM('path/to/file.osm.pbf');
  ```

- For Shapefiles / zipped shapes / GeoJSON it runs:

  ```sql
  CREATE TABLE imported_data AS
  SELECT * FROM ST_Read('path/to/file');
  ```

You can query these tables directly in the `Database / Query` tab using normal
SQL.


Helper Script
-------------

A simple helper script is provided in `bin/osm-downloader.sh`. It:

1. Verifies that `cargo` is available
2. Builds the project in release mode (`cargo build --release`)
3. Runs the compiled binary from `target/release/osm-downloader`

See the script itself for details and customisation options.


Troubleshooting
---------------

- If you see HTTP errors on download, check your network connection and the
  Geofabrik URL.
- If you see “Download incomplete: expected … bytes, got … bytes” the remote
  server or connection closed early; re-run the download.
- If DuckDB reports spatial extension errors, ensure the process has network
  access on first run so `INSTALL spatial` can fetch the extension bundle.

