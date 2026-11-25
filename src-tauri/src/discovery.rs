//! Finding OpenPres servers on the local network (mDNS / Bonjour, `_openpres._tcp`). Servers
//! announce their name, id, port, power state and version (the server's `discovery.rs`);
//! the Manager listens and lists them.

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
};

pub const SERVICE: &str = "_openpres._tcp.local.";

/// A server the Manager found.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FoundServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
    pub power: String,
    pub version: String,
    /// The oldest manager that can manage it (empty from older servers).
    pub oldest_manager: String,
}

/// Listens for servers in the background; `list` gives the ones seen so far.
pub struct Browser {
    found: Arc<Mutex<HashMap<String, FoundServer>>>,
    _daemon: Option<ServiceDaemon>,
}

impl Browser {
    pub fn start() -> Self {
        let found: Arc<Mutex<HashMap<String, FoundServer>>> = Arc::default();
        let daemon = ServiceDaemon::new().map_err(|e| tracing::warn!("discovery unavailable: {e}")).ok();
        if let Some(rx) = daemon.as_ref().and_then(|d| d.browse(SERVICE).ok()) {
            let found = found.clone();
            std::thread::Builder::new()
                .name("discovery".into())
                .spawn(move || {
                    while let Ok(event) = rx.recv() {
                        let mut found = found.lock().unwrap_or_else(|e| e.into_inner());
                        match event {
                            ServiceEvent::ServiceResolved(info) => {
                                let text = |k: &str| info.get_property_val_str(k).unwrap_or("").to_string();
                                let mut addresses: Vec<IpAddr> = info.get_addresses().iter().copied().collect();
                                // IPv4 first: the friendliest to show and type.
                                addresses.sort_by_key(|a| (a.is_ipv6(), *a));
                                found.insert(
                                    info.get_fullname().to_string(),
                                    FoundServer {
                                        id: text("id"),
                                        name: text("name"),
                                        host: info.get_hostname().trim_end_matches('.').to_string(),
                                        addresses,
                                        port: info.get_port(),
                                        power: text("power"),
                                        version: text("version"),
                                        oldest_manager: text("oldest_manager"),
                                    },
                                );
                            }
                            ServiceEvent::ServiceRemoved(_, fullname) => {
                                found.remove(&fullname);
                            }
                            _ => {}
                        }
                    }
                })
                .ok();
        }
        Self { found, _daemon: daemon }
    }

    /// The servers seen, one per id (a server reachable several ways is listed once).
    pub fn list(&self) -> Vec<FoundServer> {
        let found = self.found.lock().unwrap_or_else(|e| e.into_inner());
        let mut by_id: HashMap<String, FoundServer> = HashMap::new();
        for s in found.values() {
            by_id.entry(s.id.clone()).or_insert_with(|| s.clone());
        }
        let mut list: Vec<FoundServer> = by_id.into_values().collect();
        list.sort_by_key(|s| s.name.to_lowercase());
        list
    }
}
