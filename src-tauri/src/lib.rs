#[cfg(test)]
mod acl_tests;
pub mod commands;
pub mod install;
pub mod ipc;
pub mod local_shell;
pub mod mcp;
pub mod platform;
pub mod plugins;
pub mod remote_tools;
pub mod replay;
pub mod session;
pub mod sftp;
pub mod ssh;
pub mod storage;

use std::sync::Arc;

pub use ipc::{IpcServer, IpcServerRunError};
pub use mcp::{AgentGateway, AgentGatewayStatus, McpServer};
pub use session::SessionManager;
pub use storage::Database;

use commands::{
    add_group,
    add_server,
    agent_activity_list,
    // Agent command guard
    cancel_agent_auto_approve,
    clear_fingerprints,
    clear_sudo_credential,
    clear_sudo_policy,
    credential_status,
    delete_fingerprint,
    delete_fingerprint_by_id,
    delete_group,
    delete_server,
    get_agent_guard_config,
    get_agent_guard_status,
    get_app_version,
    // Fingerprint commands
    get_fingerprint,
    get_groups,
    get_runtime_capabilities,
    get_server_status,
    get_servers,
    list_fingerprints,
    load_settings,
    local_shell_attach,
    local_shell_create,
    local_shell_detach,
    local_shell_get_default,
    local_shell_kill,
    local_shell_kill_all,
    local_shell_list_sessions,
    // Local shell commands
    local_shell_list_shells,
    local_shell_resize,
    local_shell_send_bytes,
    local_shell_send_input,
    // LAN Agent pairing and access management
    mcp_agent_forget,
    mcp_agent_rename,
    mcp_agent_revoke,
    mcp_agent_rotate,
    mcp_agents_list,
    mcp_lan_status,
    mcp_pair_code_create,
    mcp_pair_code_pending,
    mcp_pair_request_approve,
    mcp_pair_request_reject,
    mcp_pair_requests_list,
    open_external_url,
    pick_directory_for_upload,
    pick_download_directory,
    pick_files_for_upload,
    // Dialog commands
    pick_ssh_key_file,
    pick_workspace_directory,
    probe_host_key,
    read_ssh_key_file,
    resolve_agent_approval,
    save_credential,
    save_fingerprint,
    save_settings,
    save_sudo_credential,
    save_sudo_policy,
    session_attach,
    session_connect,
    session_connect_saved,
    session_create,
    session_detach,
    session_exec_command,
    session_kill,
    session_list,
    session_resize,
    session_send_bytes,
    session_send_input,
    set_agent_guard_config,
    sftp_compress,
    sftp_delete,
    sftp_download_file,
    sftp_extract,
    sftp_get_upload_ignore_config,
    // SFTP commands
    sftp_init,
    sftp_list_dir,
    sftp_mkdir,
    sftp_pwd,
    sftp_read_file,
    sftp_rename,
    sftp_save_upload_ignore_config,
    sftp_upload_directory,
    sftp_upload_file,
    sftp_write_file,
    update_server,
    verify_fingerprint,
    FingerprintState,
    SessionAccessMode,
    SessionAccessState,
    SftpState,
};

use local_shell::LocalShellManager;
use tauri::Emitter;
use tauri::Manager;

/// Returns the current version of zxySSH
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Open developer tools (F12)
#[tauri::command]
fn open_devtools(webview: tauri::WebviewWindow) {
    webview.open_devtools();
}

#[tauri::command]
fn get_agent_gateway_status(gateway: tauri::State<'_, AgentGateway>) -> AgentGatewayStatus {
    gateway.status()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging with RUST_LOG environment variable support
    // Set RUST_LOG=debug or RUST_LOG=zxyssh=debug for detailed logs
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("[zxySSH] Starting application v{}", version());

    let builder =
        tauri::Builder::default().manage(commands::local_files::PendingOpenFiles::default());

    let builder = builder
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            commands::local_files::queue_file_arguments(app, args, std::path::Path::new(&cwd));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            commands::local_files::install_workspace_menu(app)?;
            if let Ok(cwd) = std::env::current_dir() {
                commands::local_files::queue_file_arguments(app.handle(), std::env::args(), &cwd);
            }
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;

            let primary_install = app.config().identifier == "com.zxyssh.desktop";

            let database = Arc::new(Database::new_at(platform::database_path(&app_data_dir))?);
            let session_manager = Arc::new(SessionManager::new(database.clone()));
            let sftp_state = Arc::new(SftpState::new());
            let fingerprint_state =
                FingerprintState::new_at(platform::fingerprint_path(&app_data_dir))
                    .map_err(std::io::Error::other)?;
            // SSH host-key (TOFU) verification: every connect path in the
            // session manager checks the server key against the SAME store
            // the fingerprint UI commands use (one shared instance, no stale
            // reads between the approval dialog saving a key and the
            // subsequent connect consulting it).
            session_manager.set_fingerprint_store(fingerprint_state.store.clone());
            let local_shell_manager = Arc::new(LocalShellManager::new());

            // Session reaper: only the CLI IPC loop called
            // reap_inactive_sessions and that loop never runs in the GUI, so
            // idle detached sessions lived forever. Same cadence and idle TTL
            // as ipc/socket.rs.
            {
                const SESSION_REAPER_INTERVAL: std::time::Duration =
                    std::time::Duration::from_secs(60);
                const SESSION_IDLE_TTL: std::time::Duration =
                    std::time::Duration::from_secs(30 * 60);

                let reaper = session_manager.clone();
                tauri::async_runtime::spawn(async move {
                    let mut interval = tokio::time::interval(SESSION_REAPER_INTERVAL);
                    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

                    loop {
                        interval.tick().await;
                        if let Err(err) = reaper.reap_inactive_sessions(SESSION_IDLE_TTL).await {
                            log::warn!("[SessionManager] Session reaper failed: {}", err);
                        }
                    }
                });
            }

            let session_access_state = Arc::new(SessionAccessState::new(SessionAccessMode::Local));
            session_access_state.bind_manager(&session_manager);
            let _ = session_manager
                .local_shell_manager
                .set(local_shell_manager.clone());
            if primary_install {
                // Share GUI-owned sessions with the native CLI. If a daemon
                // already owns IPC, keep its live sessions and attach to it;
                // never replace a live socket or terminate sessions on startup.
                if ipc::IpcClient::is_server_running() {
                    session_access_state.set_mode(SessionAccessMode::Remote);
                } else {
                    let db = database.clone();
                    let manager = session_manager.clone();
                    let access = session_access_state.clone();
                    std::thread::spawn(move || {
                        let server = IpcServer::new(db, manager);
                        if let Err(error) = server.run() {
                            if ipc::IpcClient::is_server_running() {
                                access.set_mode(SessionAccessMode::Remote);
                            } else {
                                log::error!("Could not start shared CLI service: {error}");
                            }
                        }
                    });
                }
            }
            let agent_input_tracker = session_manager.agent_input_tracker.clone();

            let activity_handle = app.handle().clone();
            let activity_emitter = Arc::new(move |event| {
                let _ = activity_handle.emit("agent-gateway-activity", event);
            });

            let terminal_handle = app.handle().clone();
            let terminal_input_emitter = Arc::new(move |event| {
                let _ = terminal_handle.emit("agent-terminal-input", event);
            });

            let persisted_auto_approve_until = database
                .get_setting(mcp::approval::AUTO_APPROVE_UNTIL_KEY)?
                .and_then(|value| value.parse::<i64>().ok());
            let approval_handle = app.handle().clone();
            let approval_manager = Arc::new(mcp::AgentApprovalManager::with_auto_approve_until(
                Arc::new(move |event| match event {
                    mcp::ApprovalEvent::Request(request) => {
                        let _ = approval_handle.emit("agent-approval-request", request);
                    }
                    mcp::ApprovalEvent::Resolved(resolved) => {
                        let _ = approval_handle.emit("agent-approval-resolved", resolved);
                    }
                    mcp::ApprovalEvent::State(state) => {
                        let _ = approval_handle.emit("agent-approval-state", state);
                    }
                }),
                persisted_auto_approve_until,
            ));

            let gateway = if primary_install {
                AgentGateway::start(
                    database.clone(),
                    session_manager.clone(),
                    activity_emitter,
                    terminal_input_emitter,
                    approval_manager.clone(),
                    agent_input_tracker.clone(),
                )
            } else {
                // Alternate bundle identifiers must not replace the normal
                // application's CLI/agent discovery manifest during UI QA.
                AgentGateway::start_at_path(
                    database.clone(),
                    session_manager.clone(),
                    activity_emitter,
                    terminal_input_emitter,
                    approval_manager.clone(),
                    agent_input_tracker.clone(),
                    app_data_dir.join("agent-gateway.json"),
                )
            }?;
            app.manage(gateway);
            app.manage(approval_manager);

            if std::env::var_os("ZXYSSH_SKIP_SKILL_AUTO_INSTALL").is_none() {
                std::thread::spawn(|| {
                    if let Err(error) = crate::install::install_bundled_cli_for_user() {
                        log::warn!("[Install] Could not install native CLI: {}", error);
                    }
                    for result in crate::install::install_to_all() {
                        if !result.success {
                            log::warn!(
                                "[Install] Could not install zxySSH skill for {}: {}",
                                result.tool.name,
                                result.error.unwrap_or_else(|| "unknown error".to_string())
                            );
                        }
                    }
                });
            }

            app.manage(database);
            app.manage(agent_input_tracker);
            app.manage(session_manager);
            app.manage(sftp_state);
            app.manage(fingerprint_state);
            app.manage(local_shell_manager);
            app.manage(session_access_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            agent_activity_list,
            open_devtools,
            commands::workspace_window::workspace_pointer_state,
            commands::workspace_window::workspace_exit,
            commands::workspace_window::workspace_save_handler_ready,
            open_external_url,
            get_app_version,
            get_runtime_capabilities,
            get_agent_gateway_status,
            // Agent command guard
            cancel_agent_auto_approve,
            get_agent_guard_config,
            get_agent_guard_status,
            resolve_agent_approval,
            set_agent_guard_config,
            // LAN Agent pairing: one-step code, credential management
            mcp_pair_code_create,
            mcp_pair_code_pending,
            mcp_pair_requests_list,
            mcp_pair_request_approve,
            mcp_pair_request_reject,
            mcp_agents_list,
            mcp_agent_revoke,
            mcp_agent_forget,
            mcp_agent_rename,
            mcp_agent_rotate,
            mcp_lan_status,
            // Session commands
            session_list,
            session_create,
            session_connect,
            session_connect_saved,
            session_kill,
            session_send_input,
            session_send_bytes,
            session_resize,
            session_attach,
            session_detach,
            session_exec_command,
            get_server_status,
            // Settings persistence (SQLite settings table)
            load_settings,
            save_settings,
            // Server management commands
            get_servers,
            add_server,
            update_server,
            delete_server,
            get_groups,
            add_group,
            delete_group,
            // Credential commands
            save_credential,
            clear_sudo_credential,
            clear_sudo_policy,
            credential_status,
            save_sudo_credential,
            save_sudo_policy,
            // Dialog commands
            pick_ssh_key_file,
            pick_files_for_upload,
            pick_directory_for_upload,
            pick_download_directory,
            pick_workspace_directory,
            read_ssh_key_file,
            // SFTP commands
            sftp_init,
            sftp_list_dir,
            sftp_download_file,
            sftp_upload_file,
            sftp_upload_directory,
            sftp_mkdir,
            sftp_delete,
            sftp_rename,
            sftp_pwd,
            sftp_read_file,
            sftp_write_file,
            commands::local_files::pick_local_files,
            commands::local_files::local_file_stat,
            commands::local_files::local_file_read,
            commands::local_files::local_file_write,
            commands::local_files::take_pending_open_files,
            commands::local_files::export_theme_css,
            sftp_compress,
            sftp_extract,
            sftp_get_upload_ignore_config,
            sftp_save_upload_ignore_config,
            // Fingerprint commands
            get_fingerprint,
            save_fingerprint,
            delete_fingerprint,
            delete_fingerprint_by_id,
            list_fingerprints,
            verify_fingerprint,
            clear_fingerprints,
            probe_host_key,
            // Local shell commands
            local_shell_list_shells,
            local_shell_get_default,
            local_shell_list_sessions,
            local_shell_create,
            local_shell_send_input,
            local_shell_send_bytes,
            local_shell_resize,
            local_shell_attach,
            local_shell_detach,
            local_shell_kill,
            local_shell_kill_all,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { ref urls } = event {
                commands::local_files::queue_open_files(
                    app,
                    urls.iter().filter_map(|url| url.to_file_path().ok()),
                );
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                if commands::workspace_window::save_handler_ready()
                    && app.get_webview_window("main").is_some()
                {
                    // The frontend flushes all windows, then workspace_exit uses code Some(0).
                    api.prevent_exit();
                    let _ = app.emit_to("main", "zxyssh://save-and-quit", ());
                }
            }
        });
}
