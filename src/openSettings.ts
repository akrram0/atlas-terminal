/** Opens (or focuses) the native Settings window declared in tauri.conf.json. */
export async function openSettingsWindow(): Promise<void> {
  try {
    const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
    const existing = await WebviewWindow.getByLabel('settings');
    if (existing) {
      await existing.show();
      await existing.setFocus();
      return;
    }
    // Fallback if the pre-declared window is gone (e.g. closed): recreate it.
    const win = new WebviewWindow('settings', { url: '/' });
    await win.show().catch(() => {});
    await win.setFocus().catch(() => {});
  } catch {}
}
