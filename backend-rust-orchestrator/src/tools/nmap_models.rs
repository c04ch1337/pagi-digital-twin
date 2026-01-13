use serde::Deserialize;

/// Minimal Nmap XML schema for host + port discovery.
///
/// This is intentionally partial: it only models the parts we use to build
/// [`NetworkScanResult`](backend-rust-orchestrator/src/main.rs:726).
#[derive(Debug, Deserialize)]
pub struct NmapRun {
    #[serde(rename = "host", default)]
    pub hosts: Vec<Host>,
}

#[derive(Debug, Deserialize)]
pub struct Host {
    #[serde(rename = "address", default)]
    pub addresses: Vec<Address>,

    pub status: Status,

    #[serde(rename = "hostnames", default)]
    pub hostnames: Option<Hostnames>,

    #[serde(rename = "ports", default)]
    pub ports: Option<Ports>,
}

#[derive(Debug, Deserialize)]
pub struct Address {
    #[serde(rename = "@addr")]
    pub addr: String,
    #[serde(rename = "@addrtype")]
    pub addr_type: String,
}

#[derive(Debug, Deserialize)]
pub struct Status {
    #[serde(rename = "@state")]
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct Hostnames {
    #[serde(rename = "hostname", default)]
    pub hostnames: Vec<Hostname>,
}

#[derive(Debug, Deserialize)]
pub struct Hostname {
    #[serde(rename = "@name")]
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Ports {
    #[serde(rename = "port", default)]
    pub ports: Vec<Port>,
}

#[derive(Debug, Deserialize)]
pub struct Port {
    #[serde(rename = "@portid")]
    pub portid: u16,
    #[serde(rename = "@protocol")]
    pub protocol: String,
    pub state: PortState,
    pub service: Option<Service>,
}

#[derive(Debug, Deserialize)]
pub struct PortState {
    #[serde(rename = "@state")]
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct Service {
    #[serde(rename = "@name")]
    pub name: String,
}

