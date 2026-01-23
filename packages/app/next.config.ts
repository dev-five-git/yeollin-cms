import devupApi from '@devup-api/next-plugin'
import { DevupUI } from '@devup-ui/next-plugin'
import type { NextConfig } from 'next'
import path from 'path'

const nextConfig: NextConfig = {
  // Enable React strict mode
  reactStrictMode: true,

  // Static export for embedding in Rust binary
  output: 'export',

  // Set turbopack root to parent of .yeollin/ to allow importing from app/ directory
  // This enables proxy mode where we re-export from source files for instant HMR
  // turbopack: {
  //   root: path.resolve(__dirname, '..', '..'),
  // },
  reactCompiler: true,
}

// Chain plugins: devup-ui -> devup-api -> next config
export default DevupUI(devupApi(nextConfig))
