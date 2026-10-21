import { useEffect } from 'react';
import { registerCloudSyncEntityRefresh } from '../../stores/cloudSyncCoordinator';
import { useCloudSyncStore } from '../../stores/cloudSyncStore';
import { useServerStore } from '../../stores/serverStore';

const FOREGROUND_SYNC_INTERVAL_MS = 60_000;

export function CloudSyncLifecycle() {
  const unlocked = useCloudSyncStore((state) => state.status.unlocked);
  const refreshStatus = useCloudSyncStore((state) => state.refreshStatus);
  const syncNow = useCloudSyncStore((state) => state.syncNow);
  const fetchServers = useServerStore((state) => state.fetchServers);
  const fetchGroups = useServerStore((state) => state.fetchGroups);

  useEffect(
    () => registerCloudSyncEntityRefresh(async () => {
      await Promise.all([fetchServers(), fetchGroups()]);
    }),
    [fetchGroups, fetchServers]
  );

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (!unlocked) return;

    const syncWhenActive = () => {
      if (document.visibilityState === 'visible') {
        void syncNow();
      }
    };
    const interval = window.setInterval(syncWhenActive, FOREGROUND_SYNC_INTERVAL_MS);
    document.addEventListener('visibilitychange', syncWhenActive);
    window.addEventListener('online', syncWhenActive);

    return () => {
      window.clearInterval(interval);
      document.removeEventListener('visibilitychange', syncWhenActive);
      window.removeEventListener('online', syncWhenActive);
    };
  }, [syncNow, unlocked]);

  return null;
}
