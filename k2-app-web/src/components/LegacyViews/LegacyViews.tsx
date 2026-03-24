/**
 * LegacyViews - Tauri-only features (barcode scanner, file picker, fs)
 * Not available in web version.
 */

export function SendView() {
  return (
    <div style={{ padding: '2rem', opacity: 0.5, textAlign: 'center' }}>
      This feature requires the desktop app.
    </div>
  );
}

export function ReceiveView() {
  return (
    <div style={{ padding: '2rem', opacity: 0.5, textAlign: 'center' }}>
      This feature requires the desktop app.
    </div>
  );
}

export function ContactsView() {
  return (
    <div style={{ padding: '2rem', opacity: 0.5, textAlign: 'center' }}>
      This feature requires the desktop app.
    </div>
  );
}

export function FileShareView() {
  return (
    <div style={{ padding: '2rem', opacity: 0.5, textAlign: 'center' }}>
      File sharing via QR code requires the desktop app.
    </div>
  );
}
