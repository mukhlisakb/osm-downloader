use tui_textarea::TextArea;
use crate::network::DownloadFormat;
use std::path::PathBuf;

// #[derive(Debug, PartialEq, Clone, Copy)]
// pub enum InputMode {
//     Normal,
//     Editing,
// }

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ActiveTab {
    Download,
    Database,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FocusField {
    Continent,
    Country,
    Region,
    Format,
}

pub struct App<'a> {
    pub input_continent: TextArea<'a>,
    pub input_country: TextArea<'a>,
    pub input_region: TextArea<'a>,
    pub focus_field: FocusField,
    
    pub download_format: DownloadFormat,
    pub download_progress: f64,
    pub is_downloading: bool,
    pub last_downloaded_path: Option<PathBuf>,
    pub download_status_text: String,

    pub active_tab: ActiveTab,
    
    // Database Terminal
    pub sql_input: TextArea<'a>,
    pub sql_output: String,
    #[allow(dead_code)]
    pub sql_history: Vec<String>,
    
    #[allow(dead_code)]
    pub should_quit: bool,
    pub logs: Vec<String>,
}

impl<'a> App<'a> {
    pub fn new() -> Self {
        let mut continent = TextArea::default();
        continent.set_placeholder_text("e.g. Asia");
        continent.set_block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL).title("Continent"));

        let mut country = TextArea::default();
        country.set_placeholder_text("e.g. Indonesia");
        country.set_block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL).title("Country"));

        let mut region = TextArea::default();
        region.set_placeholder_text("e.g. Kalimantan (Optional)");
        region.set_block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL).title("Region"));

        let mut sql = TextArea::default();
        sql.set_placeholder_text("SELECT * FROM downloads;");
        sql.set_block(ratatui::widgets::Block::default().borders(ratatui::widgets::Borders::ALL).title("SQL Query"));

        Self {
            input_continent: continent,
            input_country: country,
            input_region: region,
            focus_field: FocusField::Continent,
            download_format: DownloadFormat::Pbf,
            download_progress: 0.0,
            is_downloading: false,
            last_downloaded_path: None,
            download_status_text: String::from("Ready"),
            active_tab: ActiveTab::Download,
            sql_input: sql,
            sql_output: String::from("Ready to query."),
            sql_history: vec![],
            should_quit: false,
            logs: vec![],
        }
    }

    pub fn on_tick(&mut self) {}

    pub fn next_focus(&mut self) {
        self.focus_field = match self.focus_field {
            FocusField::Continent => FocusField::Country,
            FocusField::Country => FocusField::Region,
            FocusField::Region => FocusField::Format,
            FocusField::Format => FocusField::Continent,
        };
    }

    pub fn toggle_format(&mut self) {
        self.download_format = match self.download_format {
            DownloadFormat::Pbf => DownloadFormat::Shapefile,
            DownloadFormat::Shapefile => DownloadFormat::Pbf,
        };
    }

    pub fn add_log(&mut self, msg: String) {
        self.logs.push(msg);
        if self.logs.len() > 100 {
            self.logs.remove(0);
        }
    }
}
