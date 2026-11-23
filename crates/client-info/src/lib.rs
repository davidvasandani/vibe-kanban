use std::{
    net::SocketAddr,
    sync::{Arc, OnceLock},
};

/// Runtime information about the local server.
#[derive(Clone)]
pub struct ClientInfo {
    server_addr: Arc<OnceLock<SocketAddr>>,
    preview_proxy_port: Arc<OnceLock<u16>>,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientInfo {
    pub fn new() -> Self {
        Self {
            server_addr: Arc::new(OnceLock::new()),
            preview_proxy_port: Arc::new(OnceLock::new()),
        }
    }

    pub fn set_server_addr(&self, addr: SocketAddr) -> Result<(), String> {
        self.server_addr
            .set(addr)
            .map_err(|_| "server address already set".to_string())
    }

    pub fn get_server_addr(&self) -> Option<SocketAddr> {
        self.server_addr.get().copied()
    }

    pub fn set_preview_proxy_port(&self, port: u16) -> Result<(), String> {
        self.preview_proxy_port
            .set(port)
            .map_err(|_| "preview proxy port already set".to_string())
    }

    pub fn get_preview_proxy_port(&self) -> Option<u16> {
        self.preview_proxy_port.get().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clones_observe_listener_addresses_initialized_after_cloning() {
        let original = ClientInfo::new();
        let early_clone = original.clone();
        let addr = "127.0.0.1:3334".parse().unwrap();
        original.set_server_addr(addr).unwrap();
        original.set_preview_proxy_port(3335).unwrap();
        assert_eq!(early_clone.get_server_addr(), Some(addr));
        assert_eq!(early_clone.get_preview_proxy_port(), Some(3335));
        assert!(early_clone.set_server_addr(addr).is_err());
    }
}
