use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    #[serde(default)]
    pub node_ip: String,
    #[serde(default)]
    pub is_head_node: bool,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub resources_total: serde_json::Map<String, serde_json::Value>,
}

impl NodeInfo {
    pub fn role(&self) -> &'static str {
        if self.is_head_node {
            "head"
        } else {
            "worker"
        }
    }
    pub fn mem_gb(&self) -> f64 {
        self.resources_total
            .get("memory")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            / 1e9
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeUsage {
    pub cpu_used: f64,
    pub cpu_total: f64,
    pub gpu_used: f64,
    pub gpu_total: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ClusterStatus {
    pub nodes: Vec<NodeInfo>,
    pub node_usage: HashMap<String, NodeUsage>,
    pub total_cpus: f64,
    pub used_cpus: f64,
    pub total_gpus: f64,
    pub used_gpus: f64,
    pub total_mem_gb: f64,
    pub used_mem_gb: f64,
    pub total_obj_store_gb: f64,
    pub used_obj_store_gb: f64,
}

impl ClusterStatus {
    pub fn cpu_pct(&self) -> f64 {
        pct(self.used_cpus, self.total_cpus)
    }
    pub fn gpu_pct(&self) -> f64 {
        pct(self.used_gpus, self.total_gpus)
    }
    pub fn mem_pct(&self) -> f64 {
        pct(self.used_mem_gb, self.total_mem_gb)
    }
    pub fn node_cpu_pct(&self, id: &str) -> f64 {
        self.node_usage
            .get(id)
            .map(|u| pct(u.cpu_used, u.cpu_total))
            .unwrap_or(0.0)
    }
    pub fn node_gpu_pct(&self, id: &str) -> f64 {
        self.node_usage
            .get(id)
            .map(|u| pct(u.gpu_used, u.gpu_total))
            .unwrap_or(0.0)
    }
}

fn pct(used: f64, total: f64) -> f64 {
    if total > 0.0 {
        used / total * 100.0
    } else {
        0.0
    }
}

fn parse_usage_pair(val: &serde_json::Value) -> (f64, f64) {
    let arr = val.as_array();
    let used = arr
        .and_then(|a| a.first())
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let total = arr
        .and_then(|a| a.get(1))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    (used, total)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap()
}

async fn get_json(url: &str) -> Result<serde_json::Value, String> {
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_cluster_status(base_url: &str) -> Result<ClusterStatus, String> {
    let resp = get_json(&format!("{}/api/v0/nodes?view=summary", base_url)).await?;
    let nodes_val = resp
        .pointer("/data/result/result")
        .cloned()
        .unwrap_or_default();
    let nodes: Vec<NodeInfo> = serde_json::from_value(nodes_val).map_err(|e| e.to_string())?;
    let mut status = ClusterStatus {
        nodes,
        ..Default::default()
    };

    if let Ok(val) = get_json(&format!("{}/api/cluster_status", base_url)).await {
        if let Some(metrics) = val.pointer("/data/clusterStatus/loadMetricsReport") {
            apply_cluster_usage(&mut status, metrics);
            apply_per_node_usage(&mut status, metrics);
        }
    }
    Ok(status)
}

fn apply_cluster_usage(status: &mut ClusterStatus, metrics: &serde_json::Value) {
    let Some(usage) = metrics.get("usage") else {
        return;
    };
    let (uc, tc) = usage.get("CPU").map(parse_usage_pair).unwrap_or_default();
    let (ug, tg) = usage.get("GPU").map(parse_usage_pair).unwrap_or_default();
    let (um, tm) = usage
        .get("memory")
        .map(parse_usage_pair)
        .unwrap_or_default();
    let (uo, to) = usage
        .get("objectStoreMemory")
        .map(parse_usage_pair)
        .unwrap_or_default();
    status.used_cpus = uc;
    status.total_cpus = tc;
    status.used_gpus = ug;
    status.total_gpus = tg;
    status.total_mem_gb = tm / 1e9;
    status.used_mem_gb = um / 1e9;
    status.total_obj_store_gb = to / 1e9;
    status.used_obj_store_gb = uo / 1e9;
}

fn apply_per_node_usage(status: &mut ClusterStatus, metrics: &serde_json::Value) {
    let Some(by_node) = metrics.get("usageByNode").and_then(|v| v.as_object()) else {
        return;
    };
    for (node_id, usage) in by_node {
        let (cu, ct) = usage.get("CPU").map(parse_usage_pair).unwrap_or_default();
        let (gu, gt) = usage.get("GPU").map(parse_usage_pair).unwrap_or_default();
        status.node_usage.insert(
            node_id.clone(),
            NodeUsage {
                cpu_used: cu,
                cpu_total: ct,
                gpu_used: gu,
                gpu_total: gt,
            },
        );
    }
}

// ── Jobs ──

#[derive(Debug, Clone, Deserialize)]
pub struct JobInfo {
    pub job_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub entrypoint: String,
    #[serde(default)]
    pub start_time: u64,
    #[serde(default)]
    pub end_time: u64,
    #[serde(default)]
    pub driver_info: Option<DriverInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriverInfo {
    #[serde(default)]
    pub pid: String,
}

impl JobInfo {
    pub fn short_entrypoint(&self, max_len: usize) -> String {
        if self.entrypoint.len() > max_len {
            format!("{}…", &self.entrypoint[..max_len])
        } else {
            self.entrypoint.clone()
        }
    }
    pub fn duration_str(&self) -> String {
        if self.start_time == 0 {
            return "-".into();
        }
        let end_ms = if self.end_time > 0 {
            self.end_time
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        };
        let secs = end_ms.saturating_sub(self.start_time) / 1000;
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
    pub fn pid(&self) -> &str {
        self.driver_info
            .as_ref()
            .map(|d| d.pid.as_str())
            .unwrap_or("-")
    }
}

pub async fn fetch_jobs(base_url: &str) -> Result<Vec<JobInfo>, String> {
    let resp = get_json(&format!("{}/api/jobs/", base_url)).await?;
    serde_json::from_value(resp).map_err(|e| e.to_string())
}

// ── Actors ──

#[derive(Debug, Clone, Deserialize)]
pub struct ActorInfo {
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub node_id: String,
}

pub async fn fetch_actors(base_url: &str) -> Result<Vec<ActorInfo>, String> {
    let resp = get_json(&format!("{}/api/v0/actors?view=summary", base_url)).await?;
    let actors = resp
        .pointer("/data/result/result")
        .cloned()
        .unwrap_or_default();
    serde_json::from_value(actors).map_err(|e| e.to_string())
}

// ── Prometheus metrics ──

#[derive(Debug, Clone, Default)]
pub struct GpuMetric {
    pub index: u32,
    pub name: String,
    pub utilization: f64,
    pub gram_used: f64,
    pub gram_available: f64,
}

impl GpuMetric {
    pub fn gram_total(&self) -> f64 {
        self.gram_used + self.gram_available
    }
    pub fn gram_pct(&self) -> f64 {
        pct(self.gram_used, self.gram_total())
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeMetrics {
    pub gpus: Vec<GpuMetric>,
    pub cpu_util: f64,
    pub mem_used: f64,
    pub mem_total: f64,
    pub disk_used: f64,
    pub disk_free: f64,
    pub net_recv_speed: f64,
    pub net_send_speed: f64,
    pub session_name: String,
}

pub async fn fetch_metrics_targets(base_url: &str) -> Result<Vec<String>, String> {
    let resp = get_json(&format!("{}/api/prometheus/sd", base_url)).await?;
    let targets = resp
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("targets"))
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(targets)
}

pub async fn scrape_node_metrics(targets: &[String]) -> HashMap<String, NodeMetrics> {
    let c = client();
    let futures: Vec<_> = targets
        .iter()
        .map(|t| {
            let url = format!("http://{}/metrics", t);
            let c = c.clone();
            async move { c.get(&url).send().await.ok()?.text().await.ok() }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let mut best: HashMap<String, NodeMetrics> = HashMap::new();

    for text in results.into_iter().flatten() {
        let mut partial: HashMap<String, NodeMetrics> = HashMap::new();
        parse_metrics_text(&text, &mut partial);
        for (ip, m) in partial {
            let entry = best.entry(ip).or_default();
            // Only replace if new data has more GPUs, or existing has none
            if entry.gpus.is_empty() || m.gpus.len() > entry.gpus.len() {
                *entry = m;
            }
        }
    }

    for entry in best.values_mut() {
        entry.gpus.sort_by_key(|g| g.index);
    }
    best
}

fn parse_metrics_text(text: &str, map: &mut HashMap<String, NodeMetrics>) {
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some(ip) = extract_label(line, "ip") else {
            continue;
        };
        let entry = map.entry(ip).or_default();
        parse_metric_line(line, entry);
    }
}

fn parse_metric_line(line: &str, entry: &mut NodeMetrics) {
    let prefix = line.split('{').next().unwrap_or("");
    match prefix {
        "ray_node_gpus_utilization" => {
            let gpu = gpu_entry(line, &mut entry.gpus);
            gpu.name = extract_label(line, "GpuDeviceName").unwrap_or_default();
            gpu.utilization = parse_metric_value(line);
        }
        "ray_node_gram_used" => {
            gpu_entry(line, &mut entry.gpus).gram_used = parse_metric_value(line)
        }
        "ray_node_gram_available" => {
            gpu_entry(line, &mut entry.gpus).gram_available = parse_metric_value(line)
        }
        "ray_node_cpu_utilization" => {
            entry.cpu_util = parse_metric_value(line);
            if entry.session_name.is_empty() {
                entry.session_name = extract_label(line, "SessionName").unwrap_or_default();
            }
        }
        "ray_node_mem_used" => set_nonzero(&mut entry.mem_used, parse_metric_value(line)),
        "ray_node_mem_total" => set_nonzero(&mut entry.mem_total, parse_metric_value(line)),
        "ray_node_disk_usage" => set_nonzero(&mut entry.disk_used, parse_metric_value(line)),
        "ray_node_disk_free" => set_nonzero(&mut entry.disk_free, parse_metric_value(line)),
        "ray_node_network_receive_speed" => {
            set_nonzero(&mut entry.net_recv_speed, parse_metric_value(line))
        }
        "ray_node_network_send_speed" => {
            set_nonzero(&mut entry.net_send_speed, parse_metric_value(line))
        }
        _ => {}
    }
}

fn gpu_entry<'a>(line: &str, gpus: &'a mut Vec<GpuMetric>) -> &'a mut GpuMetric {
    let idx = extract_label(line, "GpuIndex")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0u32);
    get_or_insert_gpu(gpus, idx)
}

fn set_nonzero(target: &mut f64, val: f64) {
    if val > 0.0 {
        *target = val;
    }
}

fn get_or_insert_gpu(gpus: &mut Vec<GpuMetric>, idx: u32) -> &mut GpuMetric {
    if let Some(pos) = gpus.iter().position(|g| g.index == idx) {
        &mut gpus[pos]
    } else {
        gpus.push(GpuMetric {
            index: idx,
            ..Default::default()
        });
        gpus.last_mut().unwrap()
    }
}

fn extract_label(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = line.find(&pattern)? + pattern.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn parse_metric_value(line: &str) -> f64 {
    line.rsplit_once('}')
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0.0)
}

/// Parse cluster start time from session name like "session_2026-03-14_23-19-19_186273_1"
pub fn parse_session_start(session: &str) -> Option<u64> {
    // Extract "2026-03-14_23-19-19" from the session name
    let s = session.strip_prefix("session_")?;
    let datetime = s.get(..19)?; // "2026-03-14_23-19-19"
    let parts: Vec<&str> = datetime.split('_').collect();
    if parts.len() < 2 {
        return None;
    }
    let date = parts[0];
    let time = parts[1].replace('-', ":");
    let ymd: Vec<u64> = date.split('-').filter_map(|s| s.parse().ok()).collect();
    let hms: Vec<u64> = time.split(':').filter_map(|s| s.parse().ok()).collect();
    if ymd.len() != 3 || hms.len() != 3 {
        return None;
    }
    // Approximate unix timestamp
    let days = (ymd[0] - 1970) * 365 + (ymd[0] - 1969) / 4 + day_of_year(ymd[1], ymd[2]) - 1;
    Some(days * 86400 + hms[0] * 3600 + hms[1] * 60 + hms[2])
}

fn day_of_year(month: u64, day: u64) -> u64 {
    let days_before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    days_before.get(month as usize - 1).copied().unwrap_or(0) + day
}

pub fn format_uptime(session: &str) -> String {
    let Some(start) = parse_session_start(session) else {
        return String::new();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(start);
    if secs > 86400 {
        format!(
            "{}d {}h {}m",
            secs / 86400,
            (secs % 86400) / 3600,
            (secs % 3600) / 60
        )
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
