import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': { target: 'http://127.0.0.1:7800', changeOrigin: true },
      '/ws': { target: 'ws://127.0.0.1:7800', ws: true },
    },
  },
  build: {
    chunkSizeWarningLimit: 1500,
    rollupOptions: {
      output: {
        manualChunks: (id: string) => {
          if (id.includes('node_modules/@antv/')) return 'g6'
          if (
            id.includes('node_modules/antd') ||
            id.includes('node_modules/@ant-design') ||
            id.includes('node_modules/rc-') ||
            id.includes('node_modules/@rc-component')
          ) {
            return 'antd'
          }
          return undefined
        },
      },
    },
  },
})
