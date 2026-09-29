import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { NotificationProvider, applyTheme, loadTheme } from '@canonical/react-components';
import { QueryClientProvider } from '@tanstack/react-query';
import { UNAUTHORIZED_EVENT } from './lib/api/client';
import App from './App';
import { queryClient } from './app/QueryClient';
import { useAuthStore } from './stores/auth';
import './styles/index.scss';

const DARK_QUERY = '(prefers-color-scheme: dark)';

function syncTheme() {
  const theme = loadTheme();
  document.body.classList.toggle('is-dark', theme === 'dark');
  document.body.classList.toggle('is-light', theme !== 'dark');
  applyTheme(theme);
}

syncTheme();

if (typeof window !== 'undefined' && window.matchMedia) {
  window.matchMedia(DARK_QUERY).addEventListener('change', () => {
    if (loadTheme() === 'system') syncTheme();
  });
}

window.addEventListener(UNAUTHORIZED_EVENT, () => {
  useAuthStore.getState().logout();
  window.location.href = '/';
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <App />
      </NotificationProvider>
    </QueryClientProvider>
  </StrictMode>
);
