use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PortStatus {
    pub port: u16,
    pub active: bool,
    pub url: String,
    pub framework: Option<String>,
}

#[tauri::command]
fn get_local_ip() -> Result<String, String> {
    match local_ip_address::local_ip() {
        Ok(ip) => Ok(ip.to_string()),
        Err(e) => Err(format!("Failed to get local IP: {}", e)),
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

    let timeout = Duration::from_millis(400);
    let mut results = Vec::new();

    for port in ports_to_scan {
        let addr: SocketAddr = match format!("127.0.0.1:{}", port).parse() {
            Ok(a) => a,
            Err(e) => {
                results.push(PortStatus {
                    port,
                    active: false,
                    url: format!("http://{}:{}", ip, port),
                    framework: None,
                });
                eprintln!("Parse error for port {}: {}", port, e);
                continue;
            }
        };

        let is_active = match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => true,
            Err(_) => false,
        };

        let framework = if is_active {
            let url = format!("http://127.0.0.1:{}", port);
            detect_framework(&url).await
        } else {
            None
        };

        results.push(PortStatus {
            port,
            active: is_active,
            url: format!("http://{}:{}", ip, port),
            framework,
        });
    }

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
