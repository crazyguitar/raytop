use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

use super::app::{App, Focus};
use super::theme::ThemeColors;
use crate::ray::{ClusterStatus, GpuMetric, NodeInfo, NodeMetrics};

const BAR_WIDTH: usize = 20;

const HELP_KEYS: &[(&str, &str)] = &[
    ("↑ / k", "Move up"),
    ("↓ / j", "Move down"),
    ("Enter", "Toggle detail panel"),
    ("Esc", "Close detail / quit"),
    ("t", "Cycle theme"),
    ("h", "Toggle this help"),
    ("q", "Quit"),
];

// ── Helpers ──

fn bar_str(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(width.saturating_sub(filled))
    )
}

fn color_for_pct(pct: f64, tc: &ThemeColors) -> Color {
    if pct > 90.0 {
        tc.error
    } else if pct > 70.0 {
        tc.warning
    } else {
        tc.good
    }
}

fn short_id(id: &str) -> &str {
    if id.len() > 8 {
        &id[..8]
    } else {
        id
    }
}

fn styled(text: impl Into<String>, color: Color) -> Span<'static> {
    Span::styled(text.into(), Style::default().fg(color))
}

fn label_value<'a>(label: &'a str, value: String, color: Color, tc: &ThemeColors) -> [Span<'a>; 2] {
    [styled(label, tc.muted), styled(value, color)]
}

fn label_bar<'a>(label: &'a str, pct: f64, detail: String, tc: &ThemeColors) -> [Span<'a>; 2] {
    [
        styled(label, tc.muted),
        styled(
            format!("{} {detail}", bar_str(pct, 10)),
            color_for_pct(pct, tc),
        ),
    ]
}

fn state_color(state: &str, tc: &ThemeColors) -> Color {
    if state == "ALIVE" {
        tc.good
    } else {
        tc.error
    }
}

fn spaced<'a>(groups: &[&[Span<'a>]]) -> Vec<Span<'a>> {
    let mut out = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            out.push(Span::raw("  "));
        }
        out.extend_from_slice(g);
    }
    out
}

fn pct_bar_line(label: &str, pct: f64, suffix: String, tc: &ThemeColors) -> Line<'static> {
    Line::from(vec![
        styled(label, tc.muted),
        styled(
            format!("{} {suffix}", bar_str(pct, BAR_WIDTH)),
            color_for_pct(pct, tc),
        ),
    ])
}

fn fmt_bytes(b: f64) -> String {
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.1} KB", b / 1e3)
    } else {
        format!("{:.0} B", b)
    }
}
fn avg_gpu_util(m: &NodeMetrics) -> f64 {
    if m.gpus.is_empty() {
        return 0.0;
    }
    m.gpus.iter().map(|g| g.utilization).sum::<f64>() / m.gpus.len() as f64
}

fn bordered_block<'a>(title: &'a str, tc: &ThemeColors) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tc.border))
        .title(Span::styled(title, Style::default().fg(tc.header_fg)))
}

fn bold_header(cols: &[&str], tc: &ThemeColors) -> Row<'static> {
    Row::new(
        cols.iter()
            .map(|c| Cell::from(c.to_string()))
            .collect::<Vec<_>>(),
    )
    .style(
        Style::default()
            .fg(tc.header_fg)
            .add_modifier(Modifier::BOLD),
    )
}

// ── Main draw ──

pub fn draw(frame: &mut Frame, app: &mut App) {
    let tc = app.theme.colors();

    if tc.bg != Color::Reset {
        frame.render_widget(
            Block::default().style(Style::default().bg(tc.bg)),
            frame.area(),
        );
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(app.jobs_height()),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0], &tc);
    draw_jobs_table(frame, app, chunks[1], &tc);
    draw_body(frame, app, chunks[2], &tc);
    draw_status_bar(frame, app, chunks[3], &tc);

    if app.show_help {
        draw_help_popup(frame, &tc);
    }
}

fn draw_body(frame: &mut Frame, app: &mut App, area: Rect, tc: &ThemeColors) {
    if app.show_detail {
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        draw_node_table(frame, app, split[0], tc);
        draw_detail(frame, app, split[1], tc);
    } else {
        draw_node_table(frame, app, area, tc);
    }
}

// ── Header ──

fn draw_header(frame: &mut Frame, app: &App, area: Rect, tc: &ThemeColors) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tc.border))
        .title(Span::styled(
            " raytop ",
            Style::default().fg(tc.accent).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content = match (&app.status, &app.error) {
        (Some(s), _) => header_lines(s, app, tc),
        (_, Some(e)) => vec![Line::from(styled(format!("Error: {e}"), tc.error))],
        _ => vec![Line::from(styled("Connecting...", tc.muted))],
    };
    frame.render_widget(Paragraph::new(content), inner);
}

fn header_lines<'a>(s: &ClusterStatus, app: &'a App, tc: &'a ThemeColors) -> Vec<Line<'a>> {
    let uptime = app
        .node_metrics
        .values()
        .find(|m| !m.session_name.is_empty())
        .map(|m| crate::ray::format_uptime(&m.session_name))
        .unwrap_or_default();
    let nodes_str = if uptime.is_empty() {
        format!("{}", s.nodes.len())
    } else {
        format!("{}  up {}", s.nodes.len(), uptime)
    };
    let nodes = label_value("Nodes: ", nodes_str, tc.accent, tc);
    let cpu = label_bar(
        "CPU: ",
        s.cpu_pct(),
        format!("{:.0}/{:.0}", s.used_cpus, s.total_cpus),
        tc,
    );
    let gpu = label_bar(
        "GPU: ",
        s.gpu_pct(),
        format!("{:.0}/{:.0}", s.used_gpus, s.total_gpus),
        tc,
    );
    let mem = label_bar(
        "Mem: ",
        s.mem_pct(),
        format!("{:.1}/{:.1} GB", s.used_mem_gb, s.total_mem_gb),
        tc,
    );
    let obj = label_value(
        "ObjStore: ",
        format!("{:.1}/{:.1} GB", s.used_obj_store_gb, s.total_obj_store_gb),
        tc.fg,
        tc,
    );
    let ep = label_value("Endpoint: ", app.master.to_string(), tc.fg, tc);
    vec![
        Line::from(spaced(&[&nodes, &cpu])),
        Line::from(spaced(&[&gpu, &mem])),
        Line::from(spaced(&[&obj, &ep])),
    ]
}

// ── Jobs table ──

fn draw_jobs_table(frame: &mut Frame, app: &App, area: Rect, tc: &ThemeColors) {
    let total = app.running_job_count();
    let title = if total > app.visible_jobs().len() {
        format!(" Jobs ({}/{}) ", app.job_offset + 1, total)
    } else {
        " Jobs ".to_string()
    };
    let border_color = if app.focus == Focus::Jobs {
        tc.accent
    } else {
        tc.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, Style::default().fg(tc.header_fg)));
    let header = bold_header(&["Job ID", "PID", "Duration", "Entrypoint"], tc);
    let rows: Vec<Row> = app.visible_jobs().iter().map(|j| job_row(j, tc)).collect();
    let widths = [
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Min(20),
    ];
    frame.render_widget(Table::new(rows, widths).header(header).block(block), area);
}

fn job_row(j: &crate::ray::JobInfo, tc: &ThemeColors) -> Row<'static> {
    Row::new(vec![
        Cell::from(j.job_id.clone()),
        Cell::from(j.pid().to_string()),
        Cell::from(j.duration_str()).style(Style::default().fg(tc.good)),
        Cell::from(j.short_entrypoint(60)),
    ])
}

// ── Node table ──

fn draw_node_table(frame: &mut Frame, app: &mut App, area: Rect, tc: &ThemeColors) {
    let border_color = if app.focus == Focus::Nodes {
        tc.accent
    } else {
        tc.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Nodes ", Style::default().fg(tc.header_fg)));
    let header = bold_header(&["Node", "IP", "State", "CPU", "GPU", "Role"], tc);
    let rows: Vec<Row> = app.nodes().iter().map(|n| node_row(n, app, tc)).collect();
    let widths = [
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Length(7),
        Constraint::Length(20),
        Constraint::Length(20),
        Constraint::Length(7),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(Style::default().bg(tc.highlight_bg));
    let mut state = TableState::default();
    state.select(Some(app.selected_row));
    frame.render_stateful_widget(table, area, &mut state);
}

fn node_row<'a>(n: &NodeInfo, app: &App, tc: &ThemeColors) -> Row<'a> {
    let metrics = app.node_metrics.get(&n.node_ip);
    let cpu_pct = metrics.map(|m| m.cpu_util).unwrap_or(0.0);
    let gpu_pct = metrics.map(avg_gpu_util).unwrap_or(0.0);
    Row::new(vec![
        Cell::from(short_id(&n.node_id).to_string()),
        Cell::from(n.node_ip.clone()),
        Cell::from(n.state.clone()).style(Style::default().fg(state_color(&n.state, tc))),
        Cell::from(format!("{} {:.0}%", bar_str(cpu_pct, 8), cpu_pct))
            .style(Style::default().fg(color_for_pct(cpu_pct, tc))),
        Cell::from(format!("{} {:.0}%", bar_str(gpu_pct, 8), gpu_pct))
            .style(Style::default().fg(color_for_pct(gpu_pct, tc))),
        Cell::from(n.role()),
    ])
}

// ── Detail panel ──

fn draw_detail(frame: &mut Frame, app: &App, area: Rect, tc: &ThemeColors) {
    let Some(node) = app.selected_node() else {
        frame.render_widget(
            Paragraph::new("No node selected").style(Style::default().fg(tc.muted)),
            area,
        );
        return;
    };

    let title = format!(" {} ({}) ", short_id(&node.node_id), node.node_ip);
    let block = bordered_block(&title, tc);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let metrics = app.node_metrics.get(&node.node_ip);
    let mut lines = detail_resource_lines(node, metrics, tc);
    lines.push(Line::from(""));
    lines.extend(detail_gpu_lines(metrics, tc));
    lines.push(Line::from(""));
    lines.extend(detail_actor_lines(app, &node.node_id, tc));

    frame.render_widget(Paragraph::new(lines), inner);
}

fn detail_resource_lines(
    node: &NodeInfo,
    metrics: Option<&NodeMetrics>,
    tc: &ThemeColors,
) -> Vec<Line<'static>> {
    let cpu = metrics.map(|m| m.cpu_util).unwrap_or(0.0);
    let mem_used = metrics.map(|m| m.mem_used / 1e9).unwrap_or(0.0);
    let mem_total = metrics.map(|m| m.mem_total / 1e9).unwrap_or(node.mem_gb());
    let mem_pct = if mem_total > 0.0 {
        mem_used / mem_total * 100.0
    } else {
        0.0
    };
    let disk_used = metrics.map(|m| m.disk_used / 1e9).unwrap_or(0.0);
    let disk_total = metrics
        .map(|m| (m.disk_used + m.disk_free) / 1e9)
        .unwrap_or(0.0);
    let disk_pct = if disk_total > 0.0 {
        disk_used / disk_total * 100.0
    } else {
        0.0
    };
    let mut lines = vec![
        Line::from(vec![
            styled("Role: ", tc.muted),
            styled(node.role(), tc.accent),
            Span::raw("  "),
            styled("State: ", tc.muted),
            styled(node.state.clone(), state_color(&node.state, tc)),
        ]),
        Line::from(""),
        pct_bar_line("CPU  ", cpu, format!("{:.0}%", cpu), tc),
        pct_bar_line(
            "Mem  ",
            mem_pct,
            format!("{:.1}/{:.1} GB", mem_used, mem_total),
            tc,
        ),
    ];
    if disk_total > 0.0 {
        lines.push(pct_bar_line(
            "Disk ",
            disk_pct,
            format!("{:.1}/{:.1} GB", disk_used, disk_total),
            tc,
        ));
    }
    let net_rx = metrics.map(|m| m.net_recv_speed).unwrap_or(0.0);
    let net_tx = metrics.map(|m| m.net_send_speed).unwrap_or(0.0);
    if net_rx > 0.0 || net_tx > 0.0 {
        lines.push(Line::from(vec![
            styled("Net  ", tc.muted),
            styled(
                format!("↓{}/s  ↑{}/s", fmt_bytes(net_rx), fmt_bytes(net_tx)),
                tc.fg,
            ),
        ]));
    }
    lines
}

fn detail_gpu_lines(metrics: Option<&NodeMetrics>, tc: &ThemeColors) -> Vec<Line<'static>> {
    let Some(m) = metrics else {
        return waiting_line(tc);
    };
    if m.gpus.is_empty() {
        return waiting_line(tc);
    }
    m.gpus.iter().map(|g| gpu_line(g, tc)).collect()
}

fn gpu_line(g: &GpuMetric, tc: &ThemeColors) -> Line<'static> {
    Line::from(vec![
        styled(format!("GPU{:<2}", g.index), tc.muted),
        styled(
            format!(
                "{} {:>3.0}%",
                bar_str(g.utilization, BAR_WIDTH),
                g.utilization
            ),
            color_for_pct(g.utilization, tc),
        ),
        styled(
            format!("  {:.0}/{:.0}MB", g.gram_used / 1e6, g.gram_total() / 1e6),
            color_for_pct(g.gram_pct(), tc),
        ),
    ])
}

fn waiting_line(tc: &ThemeColors) -> Vec<Line<'static>> {
    vec![Line::from(styled(
        "  (waiting for GPU metrics...)",
        tc.muted,
    ))]
}

fn detail_actor_lines(app: &App, node_id: &str, tc: &ThemeColors) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(styled(" Actors ", tc.header_fg))];
    let actors = app.alive_actors_on_node(node_id);
    if actors.is_empty() {
        lines.push(Line::from(styled(" (none)", tc.muted)));
        return lines;
    }
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for a in &actors {
        *counts.entry(a.class_name.as_str()).or_default() += 1;
    }
    for (class, count) in &counts {
        lines.push(Line::from(vec![
            styled(format!("  {count}× "), tc.accent),
            styled(class.to_string(), tc.fg),
        ]));
    }
    lines
}

// ── Status bar & help ──

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect, tc: &ThemeColors) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", app.theme.label()),
                Style::default().bg(tc.status_bg).fg(tc.status_fg),
            ),
            styled(
                " h:help  q:quit  t:theme  Tab:focus  ↑↓:nav  Enter:detail ",
                tc.muted,
            ),
        ])),
        area,
    );
}

fn draw_help_popup(frame: &mut Frame, tc: &ThemeColors) {
    let popup = centered_rect(40, HELP_KEYS.len() as u16 + 4, frame.area());
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tc.accent))
        .title(Span::styled(" Help ", Style::default().fg(tc.accent)));
    let lines: Vec<Line> = HELP_KEYS
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                styled(format!("{:<12}", k), tc.accent),
                styled(*d, tc.fg),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn centered_rect(w: u16, h: u16, area: Rect) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(4));
    Rect::new(
        (area.width.saturating_sub(w)) / 2,
        (area.height.saturating_sub(h)) / 2,
        w,
        h,
    )
}
