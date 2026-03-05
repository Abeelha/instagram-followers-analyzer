use anyhow::Result;
use clap::{Arg, Command};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::path::Path;
use copypasta::{ClipboardContext, ClipboardProvider};

mod instagram;
use instagram::*;

#[derive(Clone)]
struct App {
    non_mutual_follows: Vec<String>,
    selected: usize,
    scroll_offset: usize,
}

impl App {
    fn new(non_mutual_follows: Vec<String>) -> Self {
        Self {
            non_mutual_follows,
            selected: 0,
            scroll_offset: 0,
        }
    }

    fn next(&mut self) {
        if self.selected < self.non_mutual_follows.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn adjust_scroll(&mut self, height: usize) {
        if self.selected >= self.scroll_offset + height {
            self.scroll_offset = self.selected - height + 1;
        } else if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
    }

    fn get_selected_username(&self) -> Option<&String> {
        self.non_mutual_follows.get(self.selected)
    }
}

fn main() -> Result<()> {
    let matches = Command::new("🐝 Instagram Matrix Analyzer")
        .version("0.1.0")
        .author("Matrix Bee Team")
        .about("Analyze Instagram followers with matrix-bee aesthetics")
        .arg(
            Arg::new("followers")
                .short('f')
                .long("followers")
                .value_name("FILE")
                .help("Path to followers JSON file")
                .required(false),
        )
        .arg(
            Arg::new("following")
                .short('g')
                .long("following")
                .value_name("FILE")
                .help("Path to following JSON file")
                .required(false),
        )
        .get_matches();

    let followers_path = matches.get_one::<String>("followers");
    let following_path = matches.get_one::<String>("following");

    let exe_dir = match std::env::current_exe() {
        Ok(exe_path) => exe_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf(),
        Err(_) => std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
    };

    let (followers_files, following_file) = if followers_path.is_none() || following_path.is_none() {
        let following_auto = exe_dir.join("following.json");

        let mut found: Vec<String> = Vec::new();
        let mut i = 1;
        loop {
            let candidate = exe_dir.join(format!("followers_{}.json", i));
            if candidate.exists() {
                found.push(candidate.to_string_lossy().to_string());
                i += 1;
            } else {
                break;
            }
        }

        if found.is_empty() || !following_auto.exists() {
            println!("🐝 Instagram Matrix Analyzer 🐝");
            println!();
            println!("❌ Missing Instagram data files!");
            println!();
            println!("📁 Place ALL these files next to the .exe:");
            println!("   • followers_1.json");
            println!("   • followers_2.json  (if it exists in your export)");
            println!("   • followers_3.json  (and so on...)");
            println!("   • following.json");
            println!();
            println!("📋 Steps to get your data:");
            println!("1. Go to Instagram → Settings → Privacy → Download Your Information");
            println!("2. Select 'Followers and following' data");
            println!("3. Extract and copy ALL followers_*.json files + following.json next to this .exe");
            println!();
            println!("Press Enter to exit...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            return Ok(());
        }

        println!("🐝 Found {} followers file(s) + following.json", found.len());
        println!();
        (found, following_auto.to_string_lossy().to_string())
    } else {
        (vec![followers_path.unwrap().clone()], following_path.unwrap().clone())
    };

    let non_mutual = analyze_followers(&followers_files, &following_file)?;

    if non_mutual.is_empty() {
        println!("🐝 Buzz! All your follows are mutual! Your digital hive is perfectly balanced.");
        println!();
        println!("Press Enter to exit...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        return Ok(());
    }

    println!("🐝 Found {} potential non-mutual follows", non_mutual.len());
    println!();
    println!("⚠️  Important: Some results might be false positives due to:");
    println!("   • Instagram export timing differences");
    println!("   • Missing follower files (followers_2.json, etc.)");
    println!("   • Private/restricted account limitations");
    println!();
    println!("💡 Tip: Double-check by visiting profiles before unfollowing!");
    println!();
    println!("Press Enter to continue to the interactive viewer...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();

    run_tui(non_mutual)?;
    Ok(())
}

fn run_tui(non_mutual_follows: Vec<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(non_mutual_follows);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Esc, _) => return Ok(()),
                    (KeyCode::Down, _) | (KeyCode::Char('j'), _) => app.next(),
                    (KeyCode::Up, _) | (KeyCode::Char('k'), _) => app.previous(),
                    (KeyCode::Enter, _) => {
                        if let Some(username) = app.get_selected_username() {
                            let url = format!("https://www.instagram.com/{}/", username);
                            if let Err(e) = open::that(&url) {
                                // Ignore browser open errors silently
                                eprintln!("Could not open browser: {}", e);
                            }
                        }
                    },
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        if let Some(username) = app.get_selected_username() {
                            match ClipboardContext::new() {
                                Ok(mut ctx) => {
                                    let _ = ctx.set_contents(username.to_string());
                                    // Successfully copied - could add visual feedback later
                                },
                                Err(_) => {
                                    // Silently continue if clipboard unavailable
                                }
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.size();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(size);

    let header = create_header();
    f.render_widget(header, chunks[0]);

    let list_height = chunks[1].height as usize;
    app.adjust_scroll(list_height.saturating_sub(2));

    let items: Vec<ListItem> = app
        .non_mutual_follows
        .iter()
        .skip(app.scroll_offset)
        .take(list_height)
        .enumerate()
        .map(|(i, username)| {
            let actual_index = i + app.scroll_offset;
            let style = if actual_index == app.selected {
                Style::default()
                    .bg(Color::Rgb(255, 255, 0))
                    .fg(Color::Black)
            } else {
                Style::default().fg(Color::Rgb(255, 255, 0))
            };

            ListItem::new(format!("🐝 @{}", username)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("🐝 Potential Non-Mutual Follows | Enter: Open | Ctrl+C: Copy | q: Quit")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(255, 255, 0)))
                .title_style(Style::default().fg(Color::Rgb(255, 255, 0))),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(list, chunks[1]);

    let footer = create_footer(app.non_mutual_follows.len(), app.selected + 1);
    f.render_widget(footer, chunks[2]);
}

fn create_header() -> Paragraph<'static> {
    let text = vec![Line::from(vec![
        Span::styled("🐝 ", Style::default().fg(Color::Rgb(255, 255, 0))),
        Span::styled("MATRIX", Style::default().fg(Color::Green)),
        Span::styled(" BEE ", Style::default().fg(Color::Rgb(255, 255, 0))),
        Span::styled("ANALYZER", Style::default().fg(Color::Green)),
        Span::styled(" 🐝", Style::default().fg(Color::Rgb(255, 255, 0))),
    ])];

    Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(255, 255, 0))),
        )
        .style(Style::default().bg(Color::Black))
}

fn create_footer(total: usize, current: usize) -> Paragraph<'static> {
    let text = vec![Line::from(vec![
        Span::styled("↑↓/jk: Navigate | Enter: Open | Ctrl+C: Copy | q/Esc: Quit | Total: ", Style::default().fg(Color::Gray)),
        Span::styled(total.to_string(), Style::default().fg(Color::Rgb(255, 255, 0))),
        Span::styled(" | Selected: ", Style::default().fg(Color::Gray)),
        Span::styled(current.to_string(), Style::default().fg(Color::Rgb(255, 255, 0))),
    ])];

    Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(255, 255, 0))),
        )
        .style(Style::default().bg(Color::Black))
}
