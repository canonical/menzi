import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  resolve: {
    conditions: ['sass'],
  },
  test: {
    environment: 'node',
    setupFiles: ['./vitest.setup.ts'],
  },
})