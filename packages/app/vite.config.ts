import devupApi from '@devup-api/vite-plugin'
import { DevupUI } from '@devup-ui/vite-plugin'
import vinext from 'vinext'
import { defineConfig } from 'vite'

export default defineConfig({
  base: process.env.VITE_YEOLLIN_BASE_PATH || '/',
  optimizeDeps: {
    exclude: ['@devup-ui/react'],
  },
  plugins: [
    DevupUI(),
    devupApi({ serverActions: false }),
    vinext({ react: { compiler: true } }),
    {
      name: 'yeollin-demo-base-path',
      config() {
        if (!process.env.VITE_YEOLLIN_BASE_PATH) return

        return {
          define: {
            'process.env.__NEXT_ROUTER_BASEPATH': JSON.stringify(
              process.env.VITE_YEOLLIN_BASE_PATH,
            ),
          },
        }
      },
    },
  ],
  server: {
    host: '127.0.0.1',
    ws: { path: '/__vite_hmr' },
  },
})
