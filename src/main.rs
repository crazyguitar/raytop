mod ray;
mod tui;

use clap::Parser;
use std::io;
use std::time::Duration;
use tokio::sync::watch;

#[derive(Parser)]
#[command(name = "raytop", about = "htop-like monitor for Ray clusters")]
struct Args {
    /// Ray dashboard address (e.g. http://127.0.0.1:8265)
    #[arg(long, default_value = "http://127.0.0.1:8265")]
    master: String,
}

/// All data fetched in the background.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub status: Option<ray::ClusterStatus>,
    pub jobs: Vec<ray::JobInfo>,
    pub actors: Vec<ray::ActorInfo>,
    pub node_metrics: std::collections::HashMap<String, ray::NodeMetrics>,
    pub error: Option<String>,
}

async fn fetch_loop(master: String, tx: watch::Sender<Snapshot>) {
    let mut targets: Vec<String> = Vec::new();
    loop {
        let mut snap = Snapshot::default();

        match ray::fetch_cluster_status(&master).await {
            Ok(s) => snap.status = Some(s),
            Err(e) => snap.error = Some(e),
        }

        if let Ok(mut jobs) = ray::fetch_jobs(&master).await {
            jobs.sort_by(|a, b| b.job_id.cmp(&a.job_id));
            snap.jobs = jobs;
        }
        snap.actors = ray::fetch_actors(&master).await.unwrap_or_default();

        if targets.is_empty() {
            targets = ray::fetch_metrics_targets(&master)
                .await
                .unwrap_or_default();
        }
        if !targets.is_empty() {
            snap.node_metrics = ray::scrape_node_metrics(&targets).await;
        }

        let _ = tx.send(snap);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn run_tui(master: String, rx: watch::Receiver<Snapshot>) -> io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app = tui::app::App::new(master, rx);

    loop {
        app.update();
        terminal.draw(|frame| tui::ui::draw(frame, &mut app))?;
        tui::events::handle_events(&mut app)?;
        if app.should_quit {
            break;
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    let (tx, rx) = watch::channel(Snapshot::default());

    let master = args.master.clone();
    tokio::spawn(async move { fetch_loop(master, tx).await });

    run_tui(args.master, rx)
}
