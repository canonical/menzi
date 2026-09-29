import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

const apiTarget = process.env.MENZI_API_URL || 'http://127.0.0.1:8080'
const orchestratorTarget = process.env.MENZI_ORCHESTRATOR_URL || 'http://127.0.0.1:8081'
const sessionProxyTarget = process.env.MENZI_SESSION_PROXY_URL || 'http://127.0.0.1:8082'

export default defineConfig({
  plugins: [react()],
  resolve: {
    conditions: ['sass'],
  },
  server: {
    proxy: {
      '/api/env': { target: orchestratorTarget, changeOrigin: true },
      '/api/tunnel': { target: sessionProxyTarget, changeOrigin: true },
      '/api/opencode': { target: sessionProxyTarget, changeOrigin: true },
      '/api/session': { target: sessionProxyTarget, changeOrigin: true },
      '/api/vcs': { target: sessionProxyTarget, changeOrigin: true },
      '/api/agent': { target: sessionProxyTarget, changeOrigin: true },
      '/api/model': { target: sessionProxyTarget, changeOrigin: true },
      '/api/config': { target: sessionProxyTarget, changeOrigin: true },
      '/api/event': { target: sessionProxyTarget, changeOrigin: true },
      '/api/oc/event': { target: sessionProxyTarget, changeOrigin: true },
      '/api/pty': { target: sessionProxyTarget, changeOrigin: true },
      '/api/fs': { target: sessionProxyTarget, changeOrigin: true },
      '/api/permission': { target: sessionProxyTarget, changeOrigin: true },
      '/session': { target: sessionProxyTarget, changeOrigin: true },
      '/agent': { target: sessionProxyTarget, changeOrigin: true },
      '/vcs': { target: sessionProxyTarget, changeOrigin: true },
      '/api': { target: apiTarget, changeOrigin: true },
      '/health': { target: apiTarget, changeOrigin: true },
    },
  },
  test: {
    environment: 'jsdom',
    environmentOptions: {
      jsdom: { url: 'http://localhost/' },
    },
    setupFiles: ['./vitest.setup.ts'],
  },
})