import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { NotificationProvider } from '@canonical/react-components';
import { QueryClientProvider } from '@tanstack/react-query';
import { UNAUTHORIZED_EVENT } from './lib/api/client';
import App from './App';
import { queryClient } from './app/QueryClient';
import { syncTheme, watchSystemTheme } from './lib/theme';
import { useAuthStore } from './stores/auth';
import './styles/index.scss';

syncTheme();
watchSystemTheme(syncTheme);

window.addEventListener(UNAUTHORIZED_EVENT, () => {
  useAuthStore.getState().clear();
  if (!window.location.pathname.startsWith('/login')) {
    const target = encodeURIComponent(
      window.location.pathname + window.location.search,
    );
    window.location.replace(`/login?from=${target}`);
  }
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <App />
      </NotificationProvider>
    </QueryClientProvider>
  </StrictMode>,
);
