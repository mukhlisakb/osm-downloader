use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use futures::StreamExt;
use crossterm::event::EventStream;

mod app;
mod db;
mod logging;
mod network;
mod ui;

use app::{App, ActiveTab, FocusField};
use network::{Downloader, DownloadEvent};
use db::Database;

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging
    logging::init()?;

    // Init DB
    let project_dirs = directories::ProjectDirs::from("com", "osm-downloader", "osm-downloader").unwrap();
    let data_dir = project_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;
    let db_path = data_dir.join("osm.duckdb");
    
    let db = Arc::new(Mutex::new(Database::new(&db_path)?));

    // Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App State
    let mut app = App::new();
    let downloader = Downloader::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DownloadEvent>(100);

    // Run Loop
    let res = run_app(&mut terminal, &mut app, downloader, tx, &mut rx, db).await;

    // Restore Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App<'_>,
    downloader: Downloader,
    tx: tokio::sync::mpsc::Sender<DownloadEvent>,
    rx: &mut tokio::sync::mpsc::Receiver<DownloadEvent>,
    db: Arc<Mutex<Database>>,
) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(250));
    let mut event_stream = EventStream::new();

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        tokio::select! {
            _ = interval.tick() => {
                app.on_tick();
            }
            Some(evt) = rx.recv() => {
                match evt {
                    DownloadEvent::Progress(p) => {
                        app.download_progress = p;
                        app.download_status_text = "Downloading...".to_string();
                    }
                    DownloadEvent::Complete(path) => {
                        app.download_progress = 100.0;
                        app.is_downloading = false;
                        app.download_status_text = format!("Saved to {:?}", path.file_name().unwrap());
                        app.last_downloaded_path = Some(path.clone());
                        app.add_log(format!("Download complete: {:?}", path));

                        // Auto-import to DB
                        let db_clone = db.clone();
                        let path_clone = path.clone();
                        let path_str = path_clone.to_string_lossy().to_string();
                        let url_str = "manual_download"; // Todo: track actual URL
                        let table_name = "imported_data";
                        
                        let tx_import = tx.clone(); // Clone for the background task

                        tokio::task::spawn_blocking(move || {
                            let _ = tx_import.blocking_send(DownloadEvent::ImportStarted);
                            let db = db_clone.blocking_lock();
                            let _ = db.record_download(url_str, &path_str);
                            match db.import_data(&path_str, table_name) {
                                Ok(_) => {
                                    let _ = tx_import.blocking_send(DownloadEvent::ImportFinished("Import successful.".to_string()));
                                },
                                Err(e) => {
                                    tracing::error!("Import failed: {}", e);
                                    let _ = tx_import.blocking_send(DownloadEvent::ImportFailed(e.to_string()));
                                }
                            }
                        });
                    }
                    DownloadEvent::ImportStarted => {
                        app.add_log("Starting auto-import to DuckDB...".to_string());
                    }
                    DownloadEvent::ImportFinished(msg) => {
                         app.add_log(msg);
                         // Pre-populate SQL input for convenience
                         app.sql_input = tui_textarea::TextArea::default();
                         let query = "SELECT * FROM imported_data LIMIT 10;";
                         app.sql_input.insert_str(query);
                         
                         // Auto-execute query
                         app.add_log("Auto-executing preview query...".to_string());
                         // We use blocking_lock which can panic in async context if not careful.
                         // Instead, use try_lock() to avoid blocking the runtime thread, or spawn_blocking if we really need to wait.
                         // Since we want to update UI immediately, try_lock is safer. If busy, we skip preview.
                         if let Ok(db_lock) = db.try_lock() {
                              match db_lock.query(query) {
                                   Ok(output) => app.sql_output = output,
                                   Err(e) => app.sql_output = format!("Error executing preview: {}", e),
                              }
                         } else {
                              app.add_log("DB busy, skip preview.".to_string());
                         }
                    }
                    DownloadEvent::ImportFailed(e) => {
                         app.add_log(format!("Import failed: {}", e));
                    }
                    DownloadEvent::Error(e) => {
                        app.is_downloading = false;
                        app.download_status_text = format!("Error: {}", e);
                        app.add_log(format!("Download error: {}", e));
                    }
                }
            }
            Some(Ok(event)) = event_stream.next() => {
                 match event {
                        Event::Key(key) => {
                            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                return Ok(());
                            }

                            // Global Tab Switch
                            if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                app.active_tab = match app.active_tab {
                                    ActiveTab::Download => ActiveTab::Database,
                                    ActiveTab::Database => ActiveTab::Download,
                                };
                            }

                            match app.active_tab {
                                ActiveTab::Download => {
                                    match key.code {
                                        KeyCode::Tab => app.next_focus(),
                                        KeyCode::Enter => {
                                            // Start Download
                                            if !app.is_downloading {
                                                let continent = app.input_continent.lines()[0].to_string();
                                                let country = app.input_country.lines()[0].to_string();
                                                let region = app.input_region.lines().get(0).cloned().unwrap_or_default();
                                                
                                                if continent.is_empty() {
                                                    app.add_log("Error: Continent is required".to_string());
                                                } else {
                                                    app.is_downloading = true;
                                                    app.download_progress = 0.0;
                                                    app.download_status_text = "Starting...".to_string();
                                                    app.add_log(format!("Requesting: {}/{}/{}", continent, country, region));

                                                    let url = downloader.construct_url(&continent, &country, &region, &app.download_format);
                                                    app.add_log(format!("URL: {}", url));
                                                    
                                                    let tx_clone = tx.clone();
                                                    let downloader_clone = Downloader::new(); // Cheap clone of client
                                                    
                                                    let project_dirs = directories::ProjectDirs::from("com", "osm-downloader", "osm-downloader").unwrap();
                                                    let download_dir = project_dirs.data_dir().join("downloads");
                                                    std::fs::create_dir_all(&download_dir)?;

                                                    tokio::spawn(async move {
                                                        let _ = downloader_clone.download_file(url, download_dir, tx_clone).await;
                                                    });
                                                }
                                            }
                                        }
                                        KeyCode::Char(' ') if app.focus_field == FocusField::Format => {
                                            app.toggle_format();
                                        }
                                        KeyCode::Char('q') if app.focus_field != FocusField::Continent && app.focus_field != FocusField::Country && app.focus_field != FocusField::Region => {
                                            return Ok(());
                                        }
                                        _ => {
                                            // Pass input to text areas
                                            match app.focus_field {
                                                FocusField::Continent => { app.input_continent.input(key); },
                                                FocusField::Country => { app.input_country.input(key); },
                                                FocusField::Region => { app.input_region.input(key); },
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                ActiveTab::Database => {
                                    // Log for debugging
                                    // app.add_log(format!("Key: {:?} Mod: {:?}", key.code, key.modifiers));

                                    let is_ctrl_enter = key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL);
                                    let is_ctrl_e = key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL);
                                    let is_shift_ctrl_enter = key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::SHIFT);
                                    
                                    // Also allow simple F5
                                    let is_f5 = key.code == KeyCode::F(5);

                                    if is_ctrl_enter || is_ctrl_e || is_shift_ctrl_enter || is_f5 {
                                        // Execute Query
                                        let query = app.sql_input.lines().join("\n");
                                        app.add_log(format!("Executing: {}", query));
                                        
                                        if let Ok(db_lock) = db.try_lock() {
                                            match db_lock.query(&query) {
                                                Ok(output) => app.sql_output = output,
                                                Err(e) => app.sql_output = format!("Error: {}", e),
                                            }
                                        } else {
                                            app.sql_output = "DB busy, cannot execute query.".to_string();
                                        }
                                    } else {
                                        app.sql_input.input(key);
                                    }
                                }
                            }
                        }
                        _ => {}
                     }
             }
        }
    }
}
