use anyhow::{anyhow, Result};
use duckdb::Connection;
use std::path::Path;
use tracing::{error, info};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        // Try to install and load spatial extension. 
        // This might fail if no internet or blocked, but it's required for OSM/Shapefile
        // We wrap in a result but don't fail the whole app init if it fails immediately,
        // we might fail later during import.
        if let Err(e) = conn.execute_batch("INSTALL spatial; LOAD spatial;") {
            error!("Failed to load spatial extension: {}. Import might fail.", e);
        } else {
            info!("Spatial extension loaded successfully.");
        }

        // Create a metadata table for downloads
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS downloads (
                id INTEGER PRIMARY KEY,
                url VARCHAR,
                local_path VARCHAR,
                downloaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );"
        )?;

        Ok(Self { conn })
    }

    pub fn record_download(&self, url: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO downloads (url, local_path) VALUES (?, ?)",
            [url, path],
        )?;
        Ok(())
    }

    pub fn import_data(&self, file_path: &str, table_name: &str) -> Result<()> {
        info!("Importing {} into table {}...", file_path, table_name);
        let metadata = std::fs::metadata(file_path)?;
        info!("File size for import: {} bytes", metadata.len());
        
        // Drop table if exists to overwrite
        let _ = self.conn.execute(&format!("DROP TABLE IF EXISTS {}", table_name), []);

        // Detect file type roughly by extension
        if file_path.ends_with(".osm.pbf") {
             // ST_ReadOSM logic
             // Note: ST_ReadOSM returns nodes, ways, relations. Usually complex to just "SELECT * INTO".
             // For simplicity in this tool, we might create a view or just specific tables.
             // Let's try to create a view for nodes as a default "import" action.
             let query = format!("CREATE TABLE {} AS SELECT * FROM ST_ReadOSM('{}')", table_name, file_path);
             self.conn.execute(&query, [])?;
        } else if file_path.contains(".shp") || file_path.ends_with(".zip") || file_path.ends_with(".geojson") {
            // For Shapefiles (DuckDB can read from zip directly if spatial is loaded and configured correctly, 
            // but often needs the specific .shp file inside the zip.
            // For now, let's assume the user unzipped it or we point to the .shp file.
            // If it's a zip, we might need to rely on the user to select the shp, or we handle unzip in App logic.
            // Assuming `file_path` points to a readable file for DuckDB.
            let query = format!("CREATE TABLE {} AS SELECT * FROM ST_Read('{}')", table_name, file_path);
            self.conn.execute(&query, [])?;
        } else {
            return Err(anyhow!("Unsupported file type for auto-import"));
        }

        info!("Import successful.");
        Ok(())
    }

    pub fn query(&self, sql: &str) -> Result<String> {
        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = stmt.query([])?;

        let stmt_ref = rows.as_ref().unwrap();
        let column_count = stmt_ref.column_count();
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt_ref.column_name(i).map(|s| s.to_string()).unwrap_or("unknown".to_string()))
            .collect();

        let mut rows_buffer: Vec<Vec<String>> = Vec::new();
        let mut widths: Vec<usize> = column_names.iter().map(|n| n.len()).collect();
        let mut count = 0;
        let mut truncated = false;

        while let Some(row) = rows.next()? {
            if count > 50 {
                truncated = true;
                break;
            }
            let mut values: Vec<String> = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let as_string: Result<String, _> = row.get(i);
                let mut value = match as_string {
                    Ok(s) => s,
                    Err(_) => {
                        let val = row.get_ref(i).unwrap();
                        format!("{:?}", val)
                    }
                };
                if value.len() > 80 {
                    value = format!("{}…", &value[..80]);
                }
                if value.len() > widths[i] {
                    widths[i] = value.len();
                }
                values.push(value);
            }
            rows_buffer.push(values);
            count += 1;
        }

        let max_col_width = 30;
        for w in &mut widths {
            if *w > max_col_width {
                *w = max_col_width;
            }
        }

        let mut output = String::new();

        for (idx, name) in column_names.iter().enumerate() {
            if idx > 0 {
                output.push_str(" | ");
            }
            let cell = if name.len() > widths[idx] {
                &name[..widths[idx]]
            } else {
                name
            };
            let padded = format!("{:width$}", cell, width = widths[idx]);
            output.push_str(&padded);
        }
        output.push('\n');

        for (idx, w) in widths.iter().enumerate() {
            if idx > 0 {
                output.push_str("-+-");
            }
            output.push_str(&"-".repeat(*w));
        }
        output.push('\n');

        let displayed_rows = rows_buffer.len();

        for row_values in rows_buffer {
            for (idx, value) in row_values.iter().enumerate() {
                if idx > 0 {
                    output.push_str(" | ");
                }
                let cell = if value.len() > widths[idx] {
                    &value[..widths[idx]]
                } else {
                    value
                };
                let padded = format!("{:width$}", cell, width = widths[idx]);
                output.push_str(&padded);
            }
            output.push('\n');
        }

        if truncated {
            output.push_str("... (more rows truncated)\n");
        }

        let rows_label = if truncated {
            format!("Rows: {}+ (showing {})", displayed_rows, displayed_rows)
        } else {
            format!("Rows: {}", displayed_rows)
        };
        let summary = format!("{} | Columns: {}", rows_label, column_count);
        output.push_str(&summary);
        output.push('\n');

        Ok(output)
    }
}
