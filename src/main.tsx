import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { SettingsWindow } from './components/SettingsWindow';
import './styles/globals.css';

async function bootstrap() {
  // The settings view is a separate native window running the same bundle.
  let label = 'main';
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    label = getCurrentWindow().label;
  } catch {}
  ReactDOM.createRoot(document.getElementById('root')!).render(
    <React.StrictMode>{label === 'settings' ? <SettingsWindow /> : <App />}</React.StrictMode>
  );
}

void bootstrap();
