// SPDX-FileCopyrightText: Copyright (c) 2026 owu <wqh@live.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::{AppState, AppWindow};
use crate::network;

pub fn setup(app: &AppWindow, app_handle: slint::Weak<AppWindow>, app_state: Arc<Mutex<AppState>>) {
    app.set_network_proxy_default_host(network::models::default_host().into());
    app.set_network_proxy_default_port(network::models::default_port().into());
    app.set_network_proxy_default_no_proxy(network::models::default_no_proxy().into());

    let ah = app_handle.clone();
    let as_ptr = app_state.clone();
    app.on_load_proxy_settings(move || {
        let ah = ah.clone();
        let as_ptr = as_ptr.clone();
        tokio::spawn(async move {
            let config = {
                let state = as_ptr.lock().await;
                state.config_manager.get_network_config().proxy.clone()
            };
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah.upgrade() {
                    let no_proxy = if config.no_proxy.is_empty() { network::models::default_no_proxy() } else { config.no_proxy.clone() };

                    app.set_network_proxy_is_enabled(config.is_enabled);
                    app.set_network_proxy_host(config.host.clone().into());
                    app.set_network_proxy_port(config.port.clone().into());
                    app.set_network_proxy_auth_enabled(config.auth_enabled);
                    app.set_network_proxy_username(config.username.into());
                    app.set_network_proxy_password(config.password.into());
                    app.set_network_proxy_no_proxy(no_proxy.into());
                    app.set_network_proxy_default_host(network::models::default_host().into());
                    app.set_network_proxy_default_port(network::models::default_port().into());
                    app.set_network_proxy_default_no_proxy(network::models::default_no_proxy().into());
                }
            });
        });
    });

    let ah = app_handle.clone();
    let as_ptr = app_state.clone();
    app.on_save_proxy_settings(move |enabled, host, port, no_proxy, auth, user, pass| {
        let ah = ah.clone();
        let as_ptr = as_ptr.clone();
        let host = host.to_string();
        let port_str = port.to_string();
        let no_proxy = no_proxy.to_string();
        let user = user.to_string();
        let pass = pass.to_string();

        // Validation: Port range 1-65534 (allow empty)
        let port_valid = if port_str.is_empty() {
            true
        } else if let Ok(p) = port_str.parse::<u32>() {
            p > 0 && p < 65535
        } else {
            false
        };

        if !port_valid {
            let ah_err = ah.clone();
            let ah_timer = ah.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah_err.upgrade() {
                    let err_msg = crate::i18n::t("network.proxy_error_port");
                    app.set_task_status_text(err_msg.into());
                    app.set_task_status_visible(true);
                }
                slint::Timer::single_shot(std::time::Duration::from_secs(3), move || {
                    if let Some(app) = ah_timer.upgrade() {
                        app.set_task_status_visible(false);
                    }
                });
            });
            return;
        }

        let port = port_str;
        
        // Validation: No localhost or 127.0.0.1
        if host == "localhost" || host == "127.0.0.1" {
            let ah_err = ah.clone();
            let ah_timer = ah.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah_err.upgrade() {
                    let err_msg = crate::i18n::t("network.proxy_error_localhost");
                    app.set_task_status_text(err_msg.into());
                    app.set_task_status_visible(true);
                }
                slint::Timer::single_shot(std::time::Duration::from_secs(3), move || {
                    if let Some(app) = ah_timer.upgrade() {
                        app.set_task_status_visible(false);
                    }
                });
            });
            return;
        }

        tokio::spawn(async move {
            tracing::info!("Updating HTTP proxy settings: enabled={}, host={}, port={}", enabled, host, port);
            let state = as_ptr.lock().await;
            let mut net_config = state.config_manager.get_network_config().clone();
            net_config.proxy.is_enabled = enabled;
            net_config.proxy.host = host.clone();
            net_config.proxy.port = port.clone();
            net_config.proxy.auth_enabled = auth;
            net_config.proxy.username = user.clone();
            net_config.proxy.password = pass.clone();
            net_config.proxy.no_proxy = no_proxy.clone();
            
            let _ = state.config_manager.update_network_config(net_config);
            
            let ah_status = ah.clone();
            let ah_timer = ah.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah_status.upgrade() {
                    let success_msg = crate::i18n::t("network.proxy_save_success");
                    app.set_task_status_text(success_msg.into());
                    app.set_task_status_visible(true);
                }
                slint::Timer::single_shot(std::time::Duration::from_secs(3), move || {
                    if let Some(app) = ah_timer.upgrade() {
                        app.set_task_status_visible(false);
                    }
                });
            });
        });
    });

    let ah = app_handle.clone();
    let as_ptr = app_state.clone();
    app.on_check_proxy_connection(move |url| {
        let ah = ah.clone();
        let as_ptr = as_ptr.clone();
        let url = url.to_string();
        
        let app = match ah.upgrade() {
            Some(a) => a,
            None => return,
        };
        let host = app.get_network_proxy_host().to_string();
        let port = app.get_network_proxy_port().to_string();
        let auth = app.get_network_proxy_auth_enabled();
        let user = app.get_network_proxy_username().to_string();
        let pass = app.get_network_proxy_password().to_string();
        let no_proxy = app.get_network_proxy_no_proxy().to_string();
        
        tracing::info!("Testing proxy connection to {} using proxy {}:{}", url, host, port);
        tokio::spawn(async move {
            let proxy_config = network::models::HttpProxyConfig {
                is_enabled: true,
                host: host.clone(),
                port: port.clone(),
                auth_enabled: auth,
                username: user.clone(),
                password: pass.clone(),
                no_proxy: no_proxy.clone(),
            };
            
            {
                let state = as_ptr.lock().await;
                let mut net_config = state.config_manager.get_network_config().clone();
                net_config.proxy = proxy_config.clone();
                let _ = state.config_manager.update_network_config(net_config);
            }
            
            let mut proxy_str = format!("http://{}:{}", proxy_config.host, proxy_config.port);
            
            if proxy_config.auth_enabled {
                let user_enc = urlencoding::encode(&proxy_config.username);
                let pass_enc = urlencoding::encode(&proxy_config.password);
                proxy_str = format!("http://{}:{}@{}:{}", user_enc, pass_enc, proxy_config.host, proxy_config.port);
            }
            
            let proxy_obj = match ureq::Proxy::new(&proxy_str) {
                Ok(p) => p,
                Err(e) => {
                    let ah_err = ah.clone();
                    let ah_timer = ah.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = ah_err.upgrade() {
                            let err_msg = crate::i18n::tr("network.proxy_error_invalid", &[e.to_string()]);
                            app.set_task_status_text(err_msg.into());
                            app.set_task_status_visible(true);
                        }
                        slint::Timer::single_shot(std::time::Duration::from_secs(4), move || {
                            if let Some(app) = ah_timer.upgrade() {
                                app.set_task_status_visible(false);
                            }
                        });
                    });
                    return;
                }
            };
            
            let agent = ureq::AgentBuilder::new()
                .proxy(proxy_obj)
                .timeout(std::time::Duration::from_secs(8))
                .build();
                
            let res = agent.get(&url).call();
            
            let result_msg = match res {
                Ok(response) => crate::i18n::tr("network.proxy_test_success", &[response.status().to_string()]),
                Err(e) => crate::i18n::tr("network.proxy_test_failed", &[e.to_string()]),
            };
            
            tracing::info!("Proxy test result for {}: {}", url, result_msg);
            
            let ah_status = ah.clone();
            let ah_timer = ah.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah_status.upgrade() {
                    app.set_task_status_text(result_msg.into());
                    app.set_task_status_visible(true);
                }
                slint::Timer::single_shot(std::time::Duration::from_secs(4), move || {
                    if let Some(app) = ah_timer.upgrade() {
                        app.set_task_status_visible(false);
                    }
                });
            });
        });
    });

    let ah = app_handle.clone();
    app.on_detect_proxy_host(move || {
        let ah = ah.clone();
        tokio::spawn(async move {
            tracing::info!("Auto-detecting Windows host IP from WSL network...");

            let detected_ip = detect_windows_host_ip();

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = ah.upgrade() {
                    match detected_ip {
                        Some(ip) => {
                            app.set_network_proxy_host(ip.clone().into());
                            let msg = crate::i18n::tr("network.proxy_detect_success", &[ip]);
                            app.set_task_status_text(msg.into());
                            app.set_task_status_visible(true);
                        }
                        None => {
                            let msg = crate::i18n::t("network.proxy_detect_failed");
                            app.set_task_status_text(msg.into());
                            app.set_task_status_visible(true);
                        }
                    }
                }
                let ah_timer = ah.clone();
                slint::Timer::single_shot(std::time::Duration::from_secs(4), move || {
                    if let Some(app) = ah_timer.upgrade() {
                        app.set_task_status_visible(false);
                    }
                });
            });
        });
    });
}

/// 在 WSL 内执行命令，自动探测 Windows 主机在 WSL 网络中的 IP 地址
///
/// 优先从 `/etc/resolv.conf` 的 nameserver 获取（NAT 模式下通常指向 Windows 网关），
/// 备选从默认路由的网关地址获取。
fn detect_windows_host_ip() -> Option<String> {
    use std::process::Command;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // 方法 1：从 /etc/resolv.conf 获取 nameserver
        let output = Command::new("wsl")
            .env("WSL_UTF8", "1")
            .args(["--", "cat", "/etc/resolv.conf"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = crate::wsl::decoder::decode_output(&out.stdout);
                for line in stdout.lines() {
                    let line = line.trim();
                    if line.starts_with("nameserver") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() > 1 {
                            let ip = parts[1];
                            if ip != "127.0.0.1" && !ip.is_empty() {
                                tracing::info!("Detected Windows host IP from resolv.conf: {}", ip);
                                return Some(ip.to_string());
                            }
                        }
                    }
                }
            }
        }

        // 方法 2：从默认路由获取网关地址
        let output = Command::new("wsl")
            .env("WSL_UTF8", "1")
            .args(["--", "ip", "route", "show", "default"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let stdout = crate::wsl::decoder::decode_output(&out.stdout);
                for line in stdout.lines() {
                    let line = line.trim();
                    if line.starts_with("default") {
                        // 格式：default via 172.x.x.1 dev eth0
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if let Some(via_idx) = parts.iter().position(|&s| s == "via") {
                            if via_idx + 1 < parts.len() {
                                let ip = parts[via_idx + 1];
                                if ip != "127.0.0.1" && !ip.is_empty() {
                                    tracing::info!("Detected Windows host IP from default route: {}", ip);
                                    return Some(ip.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(windows))]
    {
        // 非 Windows 平台无法执行 wsl 命令
    }

    tracing::warn!("Failed to detect Windows host IP from WSL network");
    None
}
