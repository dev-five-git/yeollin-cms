import devupApi from '@devup-api/next-plugin'
import { DevupUI } from '@devup-ui/next-plugin'
import type { NextConfig } from 'next'

const nextConfig: NextConfig = {
  // Enable React strict mode
  reactStrictMode: true,

  // Static export for embedding in Rust binary
  output: 'export',

  // Set turbopack root to this directory (avoid lockfile confusion)
  turbopack: {
    root: __dirname,
  },
  reactCompiler: true,
}

// Chain plugins: devup-ui -> devup-api -> next config
export default DevupUI(devupApi(nextConfig))
