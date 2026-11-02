import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

const host = process.env.TAURI_DEV_HOST

// Fester Port aus "Lokale Ports" (Obsidian): 1450, HMR 1451. strictPort, damit nie ein fremdes Projekt im Fenster landet.
export default defineConfig({
  plugins: [vue(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1450,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1451 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
})
