import devupApi from '@devup-api/vite-plugin'
import { DevupUI } from '@devup-ui/vite-plugin'
import vinext from 'vinext'
import { defineConfig } from 'vite'

export default defineConfig({
  optimizeDeps: {
    exclude: ['@devup-ui/react'],
  },
  plugins: [
    DevupUI(),
    devupApi({ serverActions: false }),
    vinext({ react: { compiler: true } }),
  ],
  server: {
    host: '127.0.0.1',
    ws: { path: '/__vite_hmr' },
  },
})
