use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortStatus {
    pub port: u16,
    pub active: bool,
    #[serde(rename = "networkAccessible")]
    pub network_accessible: bool,
    pub url: String,
    pub framework: Option<String>,
}

fn is_skipped_iface(name: &str) -> bool {
    let n = name.to_lowercase();
    const SKIP: &[&str] = &[
        "vpn",
        "tun",
        "tap",
        "wsl",
        "vethernet",
        "docker",
        "hyper-v",
        "vbox",
        "vmware",
        "virtual",
        "loopback",
        "isatap",
        "teredo",
        "bluetooth",
        "nordlynx",
        "tailscale",
        "wg0",
        "wireguard",
        "hamachi",
        "zerotier",
        "br-",
    ];
    SKIP.iter().any(|s| n.contains(s))
}

/// Prefer typical LAN ranges (192.168, then 10.x, then 172.16-31) and skip
/// VPN/WSL/CGNAT addresses so QR codes point at an IP phones can actually reach.
fn lan_priority(v4: std::net::Ipv4Addr) -> Option<u8> {
    if v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() || v4.is_multicast() {
        return None;
    }
    let o = v4.octets();
    // CGNAT / Tailscale 100.64.0.0/10
    if o[0] == 100 && (64..=127).contains(&o[1]) {
        return None;
    }
    if o[0] == 192 && o[1] == 168 {
        Some(0)
    } else if o[0] == 10 {
        Some(1)
    } else if o[0] == 172 && (16..=31).contains(&o[1]) {
        Some(2)
    } else {
        None
    }
}

#[tauri::command]
fn get_local_ip() -> Result<String, String> {
    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        let mut best: Option<(u8, String)> = None;
        for (name, ip) in ifaces {
            if is_skipped_iface(&name) {
                continue;
            }
            let std::net::IpAddr::V4(v4) = ip else {
                continue;
            };
            let Some(prio) = lan_priority(v4) else {
                continue;
            };
            match &best {
                None => best = Some((prio, v4.to_string())),
                Some((best_prio, _)) if prio < *best_prio => {
                    best = Some((prio, v4.to_string()));
                }
                _ => {}
            }
        }
        if let Some((_, ip)) = best {
            return Ok(ip);
        }
    }

    match local_ip_address::local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(format!("Failed to get local IP: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn lan_priority_prefers_rfc1918() {
        assert_eq!(lan_priority(Ipv4Addr::new(192, 168, 1, 15)), Some(0));
        assert_eq!(lan_priority(Ipv4Addr::new(10, 0, 0, 2)), Some(1));
        assert_eq!(lan_priority(Ipv4Addr::new(172, 16, 0, 1)), Some(2));
        assert_eq!(lan_priority(Ipv4Addr::new(100, 64, 0, 1)), None);
        assert_eq!(lan_priority(Ipv4Addr::new(8, 8, 8, 8)), None);
        assert_eq!(lan_priority(Ipv4Addr::LOCALHOST), None);
        assert_eq!(lan_priority(Ipv4Addr::new(169, 254, 1, 1)), None);
    }

    #[test]
    fn skips_virtual_ifaces() {
        assert!(is_skipped_iface("vEthernet (WSL)"));
        assert!(is_skipped_iface("NordLynx"));
        assert!(is_skipped_iface("Tailscale"));
        assert!(is_skipped_iface("Docker Bridge"));
        assert!(!is_skipped_iface("Wi-Fi"));
        assert!(!is_skipped_iface("Ethernet"));
        assert!(!is_skipped_iface("en0"));
    }
}

async fn detect_framework(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let resp = client.get(url).send().await.ok()?;
    let body = resp.text().await.ok()?;
    let body_lower = body.to_lowercase();
    let chunk = &body_lower[..body_lower.len().min(8192)];

    if chunk.contains("__next_data__") || chunk.contains("next.js") {
        Some("NEXT.JS".into())
    } else if chunk.contains("astro-island") || chunk.contains("astro-dev-toolbar") {
        Some("ASTRO".into())
    } else if chunk.contains("__nuxt") || chunk.contains("nuxt-loading") {
        Some("NUXT".into())
    } else if chunk.contains("_gatsby") || chunk.contains("gatsby") {
        Some("GATSBY".into())
    } else if chunk.contains("ng-app") || chunk.contains("@angular") {
        Some("ANGULAR".into())
    } else if chunk.contains("vite") || chunk.contains("@vite") {
        Some("VITE".into())
    } else if chunk.contains("django") {
        Some("DJANGO".into())
    } else if chunk.contains("flask") {
        Some("FLASK".into())
    } else if chunk.contains("laravel") || chunk.contains("livewire") {
        Some("LARAVEL".into())
    } else if chunk.contains("rails") {
        Some("RAILS".into())
    } else if chunk.contains("express") {
        Some("EXPRESS".into())
    } else {
        None
    }
}

#[tauri::command]
async fn scan_ports(ip: String, custom_ports: Vec<u16>) -> Result<Vec<PortStatus>, String> {
    eprintln!("[scan_ports] start ip={} custom_ports={:?}", ip, custom_ports);
    let mut ports_to_scan = vec![
        3000, 4321, 5173, 8080, 4200, 5000, 8000, 9000,
        3333, 3030, 5500, 4000, 6000, 7000, 5001, 8001,
    ];
    for p in custom_ports {
        if !ports_to_scan.contains(&p) {
            ports_to_scan.push(p);
        }
    }
    ports_to_scan.sort();

    let mut tasks = Vec::new();

    for port in ports_to_scan {
        let ip_clone = ip.clone();
        tasks.push(tokio::spawn(async move {
            let timeout_dur = Duration::from_millis(400);

            let ipv4: Option<SocketAddr> = format!("127.0.0.1:{}", port).parse().ok();
            let ipv6: Option<SocketAddr> = format!("[::1]:{}", port).parse().ok();
            let network_addr: Option<SocketAddr> = format!("{}:{}", ip_clone, port).parse().ok();

            let mut is_active = false;
            
            if let Some(addr4) = ipv4 {
                is_active = tokio::time::timeout(timeout_dur, tokio::net::TcpStream::connect(&addr4))
                    .await
                    .is_ok_and(|r| r.is_ok());
            }

            if !is_active {
                if let Some(addr6) = ipv6 {
                    is_active = tokio::time::timeout(timeout_dur, tokio::net::TcpStream::connect(&addr6))
                        .await
                        .is_ok_and(|r| r.is_ok());
                }
            }

            let mut is_network_accessible = false;
            if let Some(net_addr) = network_addr {
                is_network_accessible = tokio::time::timeout(timeout_dur, tokio::net::TcpStream::connect(&net_addr))
                    .await
                    .is_ok_and(|r| r.is_ok());
            }

            if !is_active && is_network_accessible {
                is_active = true;
            }

            let framework = if is_active {
                let mut fw = detect_framework(&format!("http://127.0.0.1:{}", port)).await;
                if fw.is_none() && ipv6.is_some() {
                    fw = detect_framework(&format!("http://[::1]:{}", port)).await;
                }
                if fw.is_none() {
                    fw = detect_framework(&format!("http://{}:{}", ip_clone, port)).await;
                }
                fw
            } else {
                None
            };

            PortStatus {
                port,
                active: is_active,
                network_accessible: is_network_accessible,
                url: format!("http://{}:{}", ip_clone, port),
                framework,
            }
        }));
    }

    let mut results = Vec::new();
    for task in tasks {
        if let Ok(status) = task.await {
            results.push(status);
        }
    }

    eprintln!("[scan_ports] done, found {} active ports", results.iter().filter(|r| r.active).count());
    Ok(results)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .invoke_handler(tauri::generate_handler![get_local_ip, scan_ports])
        .setup(|app| {
            // Hide window on autostart (starts minimized to tray)
            let args: Vec<String> = std::env::args().collect();
            let is_autostart = args.contains(&"--autostart".to_string());
            if let Some(window) = app.get_webview_window("main") {
                if is_autostart {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }

            let show_i = tauri::menu::MenuItem::with_id(
                app,
                "show",
                "Show EnvTunnel",
                true,
                None::<&str>,
            )?;
            let quit_i = tauri::menu::MenuItem::with_id(
                app,
                "quit",
                "Quit",
                true,
                None::<&str>,
            )?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_i, &quit_i])?;

            let _tray = tauri::tray::TrayIconBuilder::with_id("main")
                .tooltip("EnvTunnel")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let _ = window_clone.hide();
                        api.prevent_close();
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
