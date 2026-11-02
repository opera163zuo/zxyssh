import { useEffect, useState } from 'react';
import licenseText from '../../../LICENSE?raw';
import licenseNotice from '../../../NOTICE?raw';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../stores/settingsStore';
import { useFingerprintStore } from '../../stores/fingerprintStore';
import { useUpdateStore } from '../../stores/updateStore';
import { useRuntimeCapabilitiesStore } from '../../stores/runtimeCapabilitiesStore';
import { AgentPairingPanel } from '../AgentPairingPanel';
import {
  Globe,
  Info,
  RotateCcw,
  ExternalLink,
  Github,
  Shield,
  Key,
  FolderOpen,
  Bot,
  RefreshCw,
} from 'lucide-react';

// ============================================================================
// Reusable Form Components
// ============================================================================

interface SettingsSectionProps {
  icon: React.ReactNode;
  title: string;
  description?: string;
  children: React.ReactNode;
}

/**
 * A collapsible section container for settings groups.
 */
function SettingsSection({ icon, title, description, children }: SettingsSectionProps) {
  return (
    <section className="mb-7 min-w-0 sm:mb-8">
      <div className="mb-2 flex min-w-0 items-center gap-2 sm:gap-3">
        <span className="flex-shrink-0 text-tokyo-blue">{icon}</span>
        <h2 className="min-w-0 break-words text-lg font-semibold text-tokyo-fg sm:text-xl">{title}</h2>
      </div>
      {description && <p className="mb-4 break-words text-sm text-tokyo-comment sm:ml-8 sm:text-base">{description}</p>}
      <div className="min-w-0 space-y-4 sm:ml-8">{children}</div>
    </section>
  );
}

interface SettingRowProps {
  label: string;
  description?: string;
  children: React.ReactNode;
}

/**
 * A single setting row with label and control.
 */
function SettingRow({ label, description, children }: SettingRowProps) {
  return (
    <div className="flex min-w-0 flex-col items-stretch gap-3 border-b border-tokyo-bg-hl py-3 last:border-b-0 sm:flex-row sm:items-center sm:justify-between sm:gap-0">
      <div className="min-w-0 flex-1 sm:mr-4">
        <label className="text-tokyo-fg font-medium">{label}</label>
        {description && <p className="mt-0.5 break-words text-sm text-tokyo-comment">{description}</p>}
      </div>
      <div className="settings-control min-w-0 w-full sm:w-auto sm:flex-shrink-0">{children}</div>
    </div>
  );
}

interface SelectProps<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
}

/**
 * A styled select dropdown.
 */
function Select<T extends string>({ value, options, onChange }: SelectProps<T>) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value as T)}
      className="w-full max-w-full px-3 py-2 rounded-md bg-tokyo-bg-hl border border-tokyo-bg-hl sm:w-auto
                 text-tokyo-fg text-sm cursor-pointer
                 focus:outline-none focus:ring-1 focus:ring-tokyo-blue focus:border-tokyo-blue
                 hover:border-tokyo-comment transition-colors"
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
}

/**
 * A styled toggle switch.
 */
function Toggle({ checked, onChange }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      onClick={() => onChange(!checked)}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors
                  focus:outline-none focus:ring-2 focus:ring-tokyo-blue focus:ring-offset-2 focus:ring-offset-tokyo-bg
                  ${checked ? 'bg-tokyo-blue' : 'bg-tokyo-bg-hl'}`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform
                    ${checked ? 'translate-x-6' : 'translate-x-1'}`}
      />
    </button>
  );
}

interface TextAreaProps {
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  placeholder?: string;
}

function TextArea({ value, onChange, onBlur, placeholder }: TextAreaProps) {
  return (
    <textarea
      value={value}
      onChange={(e) => onChange(e.target.value)}
      onBlur={onBlur}
      placeholder={placeholder}
      rows={8}
      className="w-full px-3 py-2 rounded-md bg-tokyo-bg-hl border border-tokyo-bg-hl sm:w-80
                 text-tokyo-fg text-sm font-mono resize-y
                 focus:outline-none focus:ring-1 focus:ring-tokyo-blue focus:border-tokyo-blue
                 hover:border-tokyo-comment transition-colors
                 placeholder:text-tokyo-comment"
    />
  );
}

// ============================================================================
// Security Section Component
// ============================================================================

/**
 * Security settings section for managing SSH host keys and other security features.
 */
function SecuritySection() {
  const { t } = useTranslation();
  const { fingerprints, openManager, fetchFingerprints } = useFingerprintStore();

  // Fetch fingerprints on mount to show count
  useEffect(() => {
    fetchFingerprints();
  }, [fetchFingerprints]);

  return (
    <SettingsSection
      icon={<Shield className="w-5 h-5" />}
      title={t('settings.security')}
      description={t('settings.securityDesc')}
    >
      <SettingRow
        label={t('settings.hostKeyFingerprints')}
        description={t('settings.hostKeyFingerprintsDesc', { count: fingerprints.length })}
      >
        <button
          onClick={openManager}
          className="inline-flex items-center gap-2 px-4 py-2 rounded-lg
                     bg-tokyo-bg-hl text-tokyo-fg text-sm
                     hover:bg-tokyo-selection hover:text-tokyo-fg transition-colors"
        >
          <Key className="w-4 h-4" />
          <span>{t('settings.manageHostKeys')}</span>
        </button>
      </SettingRow>
    </SettingsSection>
  );
}

// ============================================================================
// Main Settings Component
// ============================================================================

/**
 * Comprehensive Settings page with Terminal, Appearance, SSH, AI Tools, and About sections.
 */
export function Settings() {
  const { t, i18n } = useTranslation();
  const {
    gatewayStatus,
    appVersion,
    appVersionLoaded,
    uploadIgnoreConfig,
    initialized,
    fetchGatewayStatus,
    fetchAppVersion,
    initializeSettings,
    updateUploadIgnoreConfig,
    resetSettings,
  } = useSettingsStore();
  const {
    currentVersion: checkedCurrentVersion,
    latestRelease,
    updateAvailable,
    checking: updateChecking,
    installing: updateInstalling,
    installProgress,
    error: updateError,
    lastCheckedAt,
    checkForUpdates,
    installLatestUpdate,
    openLatestRelease,
  } = useUpdateStore();
  const runtimeCapabilities = useRuntimeCapabilitiesStore((state) => state.capabilities);
  const loadRuntimeCapabilities = useRuntimeCapabilitiesStore((state) => state.load);

  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [uploadExcludeText, setUploadExcludeText] = useState(
    uploadIgnoreConfig.excludedPaths.join('\n')
  );

  // Load only the settings surfaces supported by the native runtime.
  useEffect(() => {
    void initializeSettings();
    void fetchAppVersion();
    void loadRuntimeCapabilities().then((capabilities) => {
      if (capabilities.agentGateway) void fetchGatewayStatus();
    });
  }, [initializeSettings, fetchGatewayStatus, fetchAppVersion, loadRuntimeCapabilities]);

  useEffect(() => {
    setUploadExcludeText(uploadIgnoreConfig.excludedPaths.join('\n'));
  }, [uploadIgnoreConfig.excludedPaths]);

  const handleResetSettings = () => {
    resetSettings();
    setShowResetConfirm(false);
  };

  const handleSaveUploadIgnores = () => {
    updateUploadIgnoreConfig({
      excludedPaths: uploadExcludeText
        .split(/\r?\n|,|;/)
        .map((value) => value.trim())
        .filter(Boolean),
    });
  };

  const effectiveAppVersion = appVersion ?? checkedCurrentVersion;
  const handleCheckUpdates = () => {
    checkForUpdates({ force: true, currentVersion: effectiveAppVersion });
  };
  const handleUpdateAction = () => {
    if (updateAvailable) {
      installLatestUpdate();
    } else {
      openLatestRelease();
    }
  };

  const lastCheckedLabel = lastCheckedAt
    ? t('settings.updateLastChecked', {
        time: new Date(lastCheckedAt).toLocaleString(i18n.language),
      })
    : t('settings.updateNotChecked');
  const updateStatus = updateError
    ? t('settings.updateCheckFailed', { message: updateError })
    : updateInstalling
      ? installProgress !== null
        ? t('settings.updateInstallingProgress', { progress: installProgress })
        : t('settings.updateInstalling')
    : updateAvailable && latestRelease
      ? t('settings.updateAvailable', { version: latestRelease.version })
      : latestRelease && effectiveAppVersion
        ? t('settings.updateUpToDate')
        : latestRelease
          ? t('settings.latestVersion', { version: latestRelease.version })
          : t('settings.updateNotChecked');
  const updateStatusClass = updateError
    ? 'text-tokyo-red'
    : updateAvailable
      ? 'text-tokyo-green'
      : 'text-tokyo-comment';

  // Show loading state while settings are initializing
  if (!initialized) {
    return (
      <div className="settings-page mx-auto w-full min-w-0 max-w-4xl overflow-x-hidden p-4 sm:p-6">
        <div className="flex items-center justify-center py-12">
          <div className="inline-block animate-spin rounded-full h-8 w-8 border-b-2 border-tokyo-blue"></div>
          <span className="ml-3 text-tokyo-comment">{t('common.loading')}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="settings-page mx-auto w-full min-w-0 max-w-4xl overflow-x-hidden p-4 sm:p-6">
      <h1 className="mb-7 text-2xl font-bold text-tokyo-fg sm:mb-8">{t('settings.title')}</h1>

      {/* ================================================================== */}
      {/* Language Section */}
      {/* ================================================================== */}
      <SettingsSection
        icon={<Globe className="w-5 h-5" />}
        title={t('settings.interface')}
        description={t('settings.interfaceDesc')}
      >
        <SettingRow label={t('settings.language')} description={t('settings.languageDesc')}>
          <Select<string>
            value={i18n.language.startsWith('zh') ? 'zh' : 'en'}
            options={[
              { value: 'en', label: 'English' },
              { value: 'zh', label: '简体中文' },
            ]}
            onChange={(lang) => i18n.changeLanguage(lang)}
          />
        </SettingRow>
      </SettingsSection>

      {/* ================================================================== */}
      {/* File Transfer Section */}
      {/* ================================================================== */}
      {runtimeCapabilities.directoryTransfer && (
        <SettingsSection
          icon={<FolderOpen className="w-5 h-5" />}
          title={t('settings.fileTransfer')}
          description={t('settings.fileTransferDesc')}
        >
        <SettingRow
          label={t('settings.respectGitignore')}
          description={t('settings.respectGitignoreDesc')}
        >
          <Toggle
            checked={uploadIgnoreConfig.respectGitignore}
            onChange={(respectGitignore) => updateUploadIgnoreConfig({ respectGitignore })}
          />
        </SettingRow>

        <SettingRow
          label={t('settings.uploadExcludePatterns')}
          description={t('settings.uploadExcludePatternsDesc')}
        >
          <TextArea
            value={uploadExcludeText}
            onChange={setUploadExcludeText}
            onBlur={handleSaveUploadIgnores}
            placeholder="node_modules/"
          />
        </SettingRow>

        <div className="flex justify-end">
          <button
            onClick={handleSaveUploadIgnores}
            className="px-4 py-2 text-sm rounded-lg bg-tokyo-blue text-tokyo-on-accent
                       hover:bg-tokyo-blue/80 transition-colors"
          >
            {t('common.save')}
          </button>
        </div>
        </SettingsSection>
      )}

      {/* ================================================================== */}
      {/* Security Section */}
      {/* ================================================================== */}
      <SecuritySection />

      {/* ================================================================== */}
      {/* Agent Gateway Section */}
      {/* ================================================================== */}
      {runtimeCapabilities.agentGateway && (
        <>
        <AgentPairingPanel />
        <SettingsSection
        icon={<Bot className="w-5 h-5" />}
        title={t('settings.gateway')}
        description={t('settings.gatewayDesc')}
      >
        <div className="border-y border-tokyo-bg-hl bg-tokyo-bg-dark/50">
          <div className="flex items-center justify-between gap-4 px-4 py-3">
            <div className="flex min-w-0 items-center gap-2">
              <span className={`h-2 w-2 flex-shrink-0 rounded-full ${gatewayStatus?.running ? 'bg-tokyo-green' : 'bg-tokyo-red'}`} />
              <div>
                <div className="text-sm font-medium text-tokyo-fg">
                  {gatewayStatus?.running ? t('settings.gatewayRunning') : t('settings.gatewayStopped')}
                </div>
                <div className="text-xs text-tokyo-comment">{t('settings.gatewaySharedGui')}</div>
              </div>
            </div>
            <button
              className="icon-button tooltip-button"
              data-tooltip={t('settings.gatewayRefresh')}
              onClick={() => fetchGatewayStatus()}
              aria-label={t('settings.gatewayRefresh')}
            >
              <RefreshCw className="h-4 w-4" />
            </button>
          </div>
          <dl className="divide-y divide-tokyo-bg-hl border-t border-tokyo-bg-hl text-sm">
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[9rem_minmax(0,1fr)]">
              <dt className="text-tokyo-comment">{t('settings.gatewayEndpoint')}</dt>
              <dd className="break-all font-mono text-xs text-tokyo-fg">{gatewayStatus?.endpoint ?? t('common.loading')}</dd>
            </div>
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[9rem_minmax(0,1fr)]">
              <dt className="text-tokyo-comment">{t('settings.gatewayManifest')}</dt>
              <dd className="break-all font-mono text-xs text-tokyo-fg">{gatewayStatus?.manifestPath ?? t('common.loading')}</dd>
            </div>
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[9rem_minmax(0,1fr)]">
              <dt className="text-tokyo-comment">{t('settings.gatewayProtocol')}</dt>
              <dd className="font-mono text-xs text-tokyo-fg">{gatewayStatus?.protocolVersion ?? t('common.loading')}</dd>
            </div>
          </dl>
        </div>
        </SettingsSection>
        </>
      )}

      {/* ================================================================== */}
      {/* About Section */}
      {/* ================================================================== */}
      <SettingsSection
        icon={<Info className="w-5 h-5" />}
        title={t('settings.about')}
        description={t('settings.aboutDesc')}
      >
        <div className="rounded-lg border border-tokyo-bg-hl bg-tokyo-bg-dark p-4 sm:p-6">
          <div className="flex min-w-0 flex-col items-start gap-3 sm:flex-row sm:gap-4">
            <div className="flex-shrink-0 text-3xl font-bold text-tokyo-blue sm:text-4xl">{'>'}_</div>
            <div className="min-w-0 flex-1">
              <h3 className="text-xl font-bold text-tokyo-fg">{t('settings.appTitle')}</h3>
              <p className="text-tokyo-comment mt-1">{t('settings.appSubtitle')}</p>

              <div className="mt-4 space-y-2">
                <div className="flex flex-wrap items-center gap-2 text-sm">
                  <span className="text-tokyo-comment">{t('common.version')}:</span>
                  <span className="text-tokyo-fg font-mono">
                    {appVersion ?? (appVersionLoaded ? t('common.unknown') : t('common.loading'))}
                  </span>
                </div>
                <div className="flex flex-wrap items-center gap-2 text-sm">
                  <span className="text-tokyo-comment">{t('common.builtWith')}:</span>
                  <span className="text-tokyo-fg">Tauri + React + TypeScript</span>
                </div>
              </div>

              <details className="mt-4 rounded-lg border border-tokyo-bg-hl p-3 text-sm">
                <summary className="cursor-pointer font-medium text-tokyo-fg">GPL-3.0-only · License / 许可证</summary>
                <p className="mt-2 text-tokyo-comment">Copyright © 2026 Rick (veithly) and contributors. No warranty. Redistribution and modification are permitted under GPLv3.</p>
                <pre className="mt-3 max-h-64 overflow-auto whitespace-pre-wrap break-words text-xs text-tokyo-fg" tabIndex={0}>{licenseNotice}{'\n'}{licenseText}</pre>
              </details>

              {runtimeCapabilities.desktopUpdater && (
                <div className="mt-4 pt-4 border-t border-tokyo-bg-hl">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="text-sm font-medium text-tokyo-fg">{t('settings.updates')}</div>
                    <div className={`text-sm mt-1 ${updateStatusClass}`}>{updateStatus}</div>
                    <div className="text-xs text-tokyo-comment mt-1">{lastCheckedLabel}</div>
                  </div>
                  <div className="flex flex-wrap gap-2">
                    <button
                      type="button"
                      onClick={handleCheckUpdates}
                      disabled={updateChecking || updateInstalling}
                      className="inline-flex items-center gap-2 px-3 py-2 rounded-lg
                                 bg-tokyo-bg-hl text-tokyo-fg text-sm
                                 hover:bg-tokyo-selection hover:text-tokyo-fg transition-colors
                                 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <RotateCcw className={`w-4 h-4 ${updateChecking ? 'animate-spin' : ''}`} />
                      <span>
                        {updateChecking ? t('settings.updateChecking') : t('settings.updateCheckNow')}
                      </span>
                    </button>
                    <button
                      type="button"
                      onClick={handleUpdateAction}
                      disabled={updateInstalling}
                      className="inline-flex items-center gap-2 px-3 py-2 rounded-lg
                                 bg-tokyo-blue text-tokyo-on-accent text-sm
                                 hover:bg-tokyo-blue/80 transition-colors
                                 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                      <ExternalLink className={`w-4 h-4 ${updateInstalling ? 'animate-pulse' : ''}`} />
                      <span>
                        {updateInstalling
                          ? t('settings.updateInstalling')
                          : updateAvailable
                            ? t('settings.updateInstall')
                            : t('settings.updateOpenLatest')}
                      </span>
                    </button>
                  </div>
                </div>
                </div>
              )}

              <div className="mt-6 flex flex-wrap gap-3">
                <a
                  href="https://github.com/opera163zuo/zxyssh"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg
                             bg-tokyo-bg-hl text-tokyo-fg text-sm
                             hover:bg-tokyo-selection hover:text-tokyo-fg transition-colors"
                >
                  <Github className="w-4 h-4" />
                  <span>GitHub</span>
                  <ExternalLink className="w-3 h-3" />
                </a>
              </div>
            </div>
          </div>
        </div>

        {/* Reset Settings */}
        <div className="mt-6 pt-6 border-t border-tokyo-bg-hl">
          <div className="flex flex-col items-stretch gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div className="min-w-0">
              <h4 className="text-tokyo-fg font-medium">{t('settings.resetSettings')}</h4>
              <p className="text-tokyo-comment text-sm mt-0.5">
                {t('settings.resetSettingsDesc')}
              </p>
            </div>
            {!showResetConfirm ? (
              <button
                onClick={() => setShowResetConfirm(true)}
                className="inline-flex items-center gap-2 px-4 py-2 rounded-lg
                           border border-tokyo-red/50 text-tokyo-red text-sm
                           hover:bg-tokyo-red/10 transition-colors"
              >
                <RotateCcw className="w-4 h-4" />
                <span>{t('settings.resetAll')}</span>
              </button>
            ) : (
              <div className="flex flex-wrap items-center gap-2">
                <span className="mr-2 text-sm text-tokyo-comment">{t('settings.resetConfirm')}</span>
                <button
                  onClick={handleResetSettings}
                  className="px-3 py-1.5 rounded-md bg-tokyo-red text-white text-sm
                             hover:bg-tokyo-red/80 transition-colors"
                >
                  {t('settings.yesReset')}
                </button>
                <button
                  onClick={() => setShowResetConfirm(false)}
                  className="px-3 py-1.5 rounded-md border border-tokyo-bg-hl text-tokyo-fg text-sm
                             hover:bg-tokyo-bg-hl transition-colors"
                >
                  {t('common.cancel')}
                </button>
              </div>
            )}
          </div>
        </div>
      </SettingsSection>
    </div>
  );
}
