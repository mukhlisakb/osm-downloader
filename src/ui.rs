use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::app::{App, ActiveTab, FocusField};
use crate::network::DownloadFormat;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title & Tabs
            Constraint::Min(0),    // Main Content
            Constraint::Length(3), // Footer / Logs
        ])
        .split(f.area());

    draw_header_tabs(f, app, chunks[0]);
    
    match app.active_tab {
        ActiveTab::Download => draw_download_tab(f, app, chunks[1]),
        ActiveTab::Database => draw_database_tab(f, app, chunks[1]),
    }

    draw_footer(f, app, chunks[2]);
}

fn draw_header_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["Download", "Database / Query"];
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title("OSM Downloader"))
        .select(match app.active_tab {
            ActiveTab::Download => 0,
            ActiveTab::Database => 1,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn draw_download_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Continent
            Constraint::Length(3), // Country
            Constraint::Length(3), // Region
            Constraint::Length(3), // Format
            Constraint::Length(3), // Progress
            Constraint::Min(0),    // Instructions/Space
        ])
        .margin(1)
        .split(area);

    // Inputs
    let active_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(Color::White);

    // Continent
    app.input_continent.set_style(if app.focus_field == FocusField::Continent { active_style } else { inactive_style });
    app.input_continent.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("Continent (e.g. Asia)")
            .style(if app.focus_field == FocusField::Continent { active_style } else { inactive_style })
    );
    f.render_widget(&app.input_continent, chunks[0]);

    // Country
    app.input_country.set_style(if app.focus_field == FocusField::Country { active_style } else { inactive_style });
    app.input_country.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("Country (e.g. Indonesia)")
            .style(if app.focus_field == FocusField::Country { active_style } else { inactive_style })
    );
    f.render_widget(&app.input_country, chunks[1]);

    // Region
    app.input_region.set_style(if app.focus_field == FocusField::Region { active_style } else { inactive_style });
    app.input_region.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title("Region (Optional, e.g. Kalimantan)")
            .style(if app.focus_field == FocusField::Region { active_style } else { inactive_style })
    );
    f.render_widget(&app.input_region, chunks[2]);

    // Format Selection
    let format_text = match app.download_format {
        DownloadFormat::Pbf => "(*) OSM PBF (.osm.pbf)   ( ) Shapefile (.shp.zip)",
        DownloadFormat::Shapefile => "( ) OSM PBF (.osm.pbf)   (*) Shapefile (.shp.zip)",
    };
    let format_p = Paragraph::new(format_text)
        .block(Block::default().borders(Borders::ALL).title("Format (Press Space to Toggle)"))
        .style(if app.focus_field == FocusField::Format { active_style } else { inactive_style });
    f.render_widget(format_p, chunks[3]);

    // Progress Bar
    let label = format!("{:.1}% - {}", app.download_progress, app.download_status_text);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(app.download_progress / 100.0)
        .label(label);
    f.render_widget(gauge, chunks[4]);

    // Help text
    let help_text = "Tab: Switch Field | Enter: Download | Ctrl+b: Switch Tabs | q: Quit";
    let help = Paragraph::new(help_text).style(Style::default().fg(Color::Gray));
    f.render_widget(help, chunks[5]);
}

fn draw_database_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Input
            Constraint::Min(0),     // Output
        ])
        .margin(1)
        .split(area);

    let active_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    app.sql_input.set_style(active_style);
    app.sql_input.set_block(Block::default().borders(Borders::ALL).title("SQL Query (Press Ctrl+e to Execute)").style(active_style));
    f.render_widget(&app.sql_input, chunks[0]);

    let output = Paragraph::new(app.sql_output.as_str())
        .block(Block::default().borders(Borders::ALL).title("Result"))
        .wrap(Wrap { trim: false });
    f.render_widget(output, chunks[1]);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let last_log = app.logs.last().map(|s| s.as_str()).unwrap_or("Ready.");
    let p = Paragraph::new(Line::from(vec![
        Span::raw("LOG: "),
        Span::styled(last_log, Style::default().fg(Color::Cyan)),
    ])).block(Block::default().borders(Borders::TOP));
    f.render_widget(p, area);
}
