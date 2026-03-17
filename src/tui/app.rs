use std::collections::HashMap;
use tokio::sync::watch;

use super::theme::Theme;
use crate::ray::{ActorInfo, ClusterStatus, JobInfo, NodeInfo, NodeMetrics};
use crate::Snapshot;

const MAX_VISIBLE_JOBS: usize = 16;

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Nodes,
    Jobs,
}

pub struct App {
    pub should_quit: bool,
    pub master: String,
    pub status: Option<ClusterStatus>,
    pub jobs: Vec<JobInfo>,
    pub actors: Vec<ActorInfo>,
    pub node_metrics: HashMap<String, NodeMetrics>,
    pub error: Option<String>,
    pub selected_row: usize,
    pub job_offset: usize,
    pub focus: Focus,
    pub show_detail: bool,
    pub show_help: bool,
    pub theme: Theme,
    rx: watch::Receiver<Snapshot>,
}

impl App {
    pub fn new(master: String, rx: watch::Receiver<Snapshot>) -> Self {
        Self {
            should_quit: false,
            master,
            status: None,
            jobs: Vec::new(),
            actors: Vec::new(),
            node_metrics: HashMap::new(),
            error: None,
            selected_row: 0,
            job_offset: 0,
            focus: Focus::Nodes,
            show_detail: false,
            show_help: false,
            theme: Theme::Default,
            rx,
        }
    }

    pub fn update(&mut self) {
        if self.rx.has_changed().unwrap_or(false) {
            let snap = self.rx.borrow_and_update().clone();
            self.status = snap.status;
            self.jobs = snap.jobs;
            self.actors = snap.actors;
            self.error = snap.error;
            for (ip, m) in snap.node_metrics {
                let dominated = self
                    .node_metrics
                    .get(&ip)
                    .map(|old| !old.gpus.is_empty() && m.gpus.is_empty())
                    .unwrap_or(false);
                if !dominated {
                    self.node_metrics.insert(ip, m);
                }
            }
            self.clamp_selection();
        }
    }

    pub fn nodes(&self) -> &[NodeInfo] {
        self.status
            .as_ref()
            .map(|s| s.nodes.as_slice())
            .unwrap_or(&[])
    }

    pub fn selected_node(&self) -> Option<&NodeInfo> {
        self.nodes().get(self.selected_row)
    }

    pub fn alive_actors_on_node(&self, node_id: &str) -> Vec<&ActorInfo> {
        self.actors
            .iter()
            .filter(|a| a.state == "ALIVE" && a.node_id == node_id)
            .collect()
    }

    pub fn running_jobs(&self) -> Vec<&JobInfo> {
        self.jobs.iter().filter(|j| j.status == "RUNNING").collect()
    }

    pub fn running_job_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.status == "RUNNING").count()
    }

    pub fn visible_jobs(&self) -> Vec<&JobInfo> {
        self.running_jobs()
            .into_iter()
            .skip(self.job_offset)
            .take(MAX_VISIBLE_JOBS)
            .collect()
    }

    pub fn jobs_height(&self) -> u16 {
        let count = self.running_job_count().min(MAX_VISIBLE_JOBS);
        3 + count as u16 // border + header + rows
    }

    pub fn move_up(&mut self) {
        match self.focus {
            Focus::Nodes => self.selected_row = self.selected_row.saturating_sub(1),
            Focus::Jobs => self.job_offset = self.job_offset.saturating_sub(1),
        }
    }

    pub fn move_down(&mut self) {
        match self.focus {
            Focus::Nodes => {
                let len = self.nodes().len();
                if len > 0 && self.selected_row < len - 1 {
                    self.selected_row += 1;
                }
            }
            Focus::Jobs => {
                let total = self.running_job_count();
                if total > MAX_VISIBLE_JOBS && self.job_offset < total - MAX_VISIBLE_JOBS {
                    self.job_offset += 1;
                }
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Nodes => Focus::Jobs,
            Focus::Jobs => Focus::Nodes,
        };
    }

    pub fn toggle_detail(&mut self) {
        self.show_detail = !self.show_detail;
    }
    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
    }

    fn clamp_selection(&mut self) {
        let len = self.nodes().len();
        if len > 0 && self.selected_row >= len {
            self.selected_row = len - 1;
        }
        let total = self.running_job_count();
        if total > MAX_VISIBLE_JOBS && self.job_offset > total - MAX_VISIBLE_JOBS {
            self.job_offset = total - MAX_VISIBLE_JOBS;
        }
    }
}
