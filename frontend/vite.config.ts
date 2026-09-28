import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

const apiTarget = process.env.MENZI_API_URL || 'http://127.0.0.1:8080'

export default defineConfig({
  plugins: [react()],
  resolve: {
    conditions: ['sass'],
  },
  server: {
    proxy: {
      '/api': { target: apiTarget, changeOrigin: true },
      '/health': { target: apiTarget, changeOrigin: true },
    },
  },
  test: {
    environment: 'node',
    setupFiles: ['./vitest.setup.ts'],
  },
})