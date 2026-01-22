import devupApi from '@devup-api/next-plugin'
import { DevupUI } from '@devup-ui/next-plugin'
import type { NextConfig } from 'next'

const nextConfig: NextConfig = {
  // Enable React strict mode
  reactStrictMode: true,

  // Set turbopack root to this directory (avoid lockfile confusion)
  turbopack: {
    root: __dirname,
  },

  // Environment variables
  env: {
    API_URL: process.env.API_URL || 'http://localhost:3001',
  },
}

// Chain plugins: devup-ui -> devup-api -> next config
export default DevupUI(devupApi(nextConfig))
