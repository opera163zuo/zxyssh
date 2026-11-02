import { create } from 'zustand';
import { invokeOrThrow } from '../lib/tauri';

// ============================================================================
// Types
// ============================================================================

/** Status of the Agent Gateway hosted by the zxySSH GUI. */
export interface AgentGatewayStatus {
  running: boolean;
  endpoint: string | null;
  manifestPath: string;
  pid: number | null;
  protocolVersion: string;
}

/**
 * Global ignore rules for recursive SFTP uploads and syncs.
 */
export interface UploadIgnoreConfig {
  /** Exclude patterns, one per line in the settings UI */
  excludedPaths: string[];
  /** Whether .gitignore files under the upload root should be honored */
  respectGitignore: boolean;
}

/**
 * Default recursive upload ignore config.
 */
export const defaultUploadIgnoreConfig: UploadIgnoreConfig = {
  excludedPaths: [
    'node_modules/',
    '.git/',
    '.svn/',
    '.hg/',
    'target/',
    '.next/',
    '.nuxt/',
    '.turbo/',
    '.cache/',
    'coverage/',
    '__pycache__/',
    '.pytest_cache/',
    '.mypy_cache/',
    '.ruff_cache/',
    '.venv/',
    'venv/',
    'env/',
  ],
  respectGitignore: true,
};

// ============================================================================
// Store Interface
// ============================================================================

interface SettingsStore {
  /** Built-in Agent Gateway status */
  gatewayStatus: AgentGatewayStatus | null;
  /** Current application version from the backend package metadata */
  appVersion: string | null;
  /** Whether app version lookup has completed */
  appVersionLoaded: boolean;

  /** Global recursive upload ignore config */
  uploadIgnoreConfig: UploadIgnoreConfig;

  // Loading/Error state
  /** Loading state for async operations */
  loading: boolean;
  /** Error message if any operation fails */
  error: string | null;
  /** Whether settings have been loaded from storage */
  initialized: boolean;

  /** Fetch built-in Agent Gateway status */
  fetchGatewayStatus: () => Promise<void>;
  /** Fetch current app version */
  fetchAppVersion: () => Promise<void>;

  // Settings actions
  /** Initialize settings from persistent storage */
  initializeSettings: () => Promise<void>;
  /** Update global recursive upload ignore config */
  updateUploadIgnoreConfig: (config: Partial<UploadIgnoreConfig>) => Promise<void>;
  /** Reset all settings to defaults */
  resetSettings: () => Promise<void>;
}

// ============================================================================
// Persistence Helpers
// ============================================================================

async function loadUploadIgnoreConfig(): Promise<UploadIgnoreConfig> {
  try {
    const config = await invokeOrThrow<UploadIgnoreConfig | null>('sftp_get_upload_ignore_config');
    if (config) {
      return {
        excludedPaths: config.excludedPaths ?? defaultUploadIgnoreConfig.excludedPaths,
        respectGitignore: config.respectGitignore ?? defaultUploadIgnoreConfig.respectGitignore,
      };
    }
  } catch (error) {
    console.warn('Failed to load upload ignore config:', error);
  }
  return { ...defaultUploadIgnoreConfig };
}

async function saveUploadIgnoreConfig(config: UploadIgnoreConfig): Promise<UploadIgnoreConfig> {
  try {
    return await invokeOrThrow<UploadIgnoreConfig>('sftp_save_upload_ignore_config', { config });
  } catch (error) {
    console.warn('Failed to save upload ignore config:', error);
    throw error;
  }
}

// ============================================================================
// Store Implementation
// ============================================================================

/**
 * Zustand store for what an operator can still tune. Terminal presentation and
 * the colour theme are fixed in code, so only the transfer ignore rules and the
 * read-only status fields live here.
 */
export const useSettingsStore = create<SettingsStore>((set, get) => ({
  // Initial state
  gatewayStatus: null,
  appVersion: null,
  appVersionLoaded: false,
  uploadIgnoreConfig: { ...defaultUploadIgnoreConfig },
  loading: false,
  error: null,
  initialized: false,

  fetchGatewayStatus: async () => {
    try {
      const status = await invokeOrThrow<AgentGatewayStatus>('get_agent_gateway_status');
      set({ gatewayStatus: status });
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : String(error),
      });
    }
  },

  fetchAppVersion: async () => {
    try {
      const appVersion = await invokeOrThrow<string>('get_app_version');
      set({ appVersion, appVersionLoaded: true });
    } catch (error) {
      console.warn('Failed to fetch app version:', error);
      set({ appVersion: null, appVersionLoaded: true });
    }
  },

  // -------------------------------------------------------------------------
  // Settings Actions
  // -------------------------------------------------------------------------

  initializeSettings: async () => {
    if (get().initialized) return;

    try {
      const uploadIgnoreConfig = await loadUploadIgnoreConfig();
      set({ uploadIgnoreConfig, initialized: true });
    } catch (error) {
      console.error('Failed to initialize settings:', error);
      set({ initialized: true }); // Mark as initialized even on error to prevent infinite loops
    }
  },

  updateUploadIgnoreConfig: async (uploadIgnoreUpdates: Partial<UploadIgnoreConfig>) => {
    const currentConfig = get().uploadIgnoreConfig;
    const nextConfig: UploadIgnoreConfig = {
      ...currentConfig,
      ...uploadIgnoreUpdates,
      excludedPaths:
        uploadIgnoreUpdates.excludedPaths?.map((value) => value.trim()).filter(Boolean) ??
        currentConfig.excludedPaths,
    };

    set({ uploadIgnoreConfig: nextConfig });
    const savedConfig = await saveUploadIgnoreConfig(nextConfig);
    set({ uploadIgnoreConfig: savedConfig });
  },

  resetSettings: async () => {
    set({ uploadIgnoreConfig: { ...defaultUploadIgnoreConfig } });
    await saveUploadIgnoreConfig(defaultUploadIgnoreConfig);
  },
}));
