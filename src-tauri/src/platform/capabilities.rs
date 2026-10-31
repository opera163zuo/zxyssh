use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimePlatform {
    Macos,
    Linux,
    Unknown,
}

impl RuntimePlatform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Unknown
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub platform: RuntimePlatform,
    pub window_controls: bool,
    pub local_shell: bool,
    pub agent_gateway: bool,
    pub desktop_updater: bool,
    pub directory_transfer: bool,
}

impl RuntimeCapabilities {
    pub fn current() -> Self {
        Self::for_platform(RuntimePlatform::current())
    }

    fn for_platform(platform: RuntimePlatform) -> Self {
        let is_desktop = matches!(platform, RuntimePlatform::Macos | RuntimePlatform::Linux);

        Self {
            platform,
            window_controls: is_desktop,
            local_shell: is_desktop,
            agent_gateway: is_desktop,
            desktop_updater: is_desktop,
            directory_transfer: is_desktop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_platforms_enable_desktop_only_capabilities() {
        let capabilities = RuntimeCapabilities::for_platform(RuntimePlatform::Linux);

        assert!(capabilities.window_controls);
        assert!(capabilities.local_shell);
        assert!(capabilities.agent_gateway);
        assert!(capabilities.desktop_updater);
        assert!(capabilities.directory_transfer);
    }

    #[test]
    fn unknown_platforms_disable_desktop_only_capabilities() {
        let capabilities = RuntimeCapabilities::for_platform(RuntimePlatform::Unknown);

        assert!(!capabilities.window_controls);
        assert!(!capabilities.local_shell);
        assert!(!capabilities.agent_gateway);
        assert!(!capabilities.desktop_updater);
        assert!(!capabilities.directory_transfer);
    }

    #[test]
    fn serializes_for_the_frontend_contract() {
        let value = serde_json::to_value(RuntimeCapabilities::for_platform(RuntimePlatform::Macos))
            .unwrap();

        assert_eq!(value["platform"], "macos");
        assert_eq!(value["windowControls"], true);
        assert_eq!(value["localShell"], true);
        assert_eq!(value["agentGateway"], true);
        assert_eq!(value["desktopUpdater"], true);
        assert_eq!(value["directoryTransfer"], true);
        assert!(
            value.get("isMobile").is_none(),
            "the mobile flag went away with the mobile targets"
        );
    }
}
