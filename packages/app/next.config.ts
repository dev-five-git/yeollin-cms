import type { NextConfig } from 'vinext'

import { STATIC_EXPORT_DEPLOYMENT_ID } from './src/lib/static-export-rsc-transport'

const isPagesDemo = process.env.VITE_YEOLLIN_DEMO === 'true'

const nextConfig: NextConfig = {
  assetPrefix: isPagesDemo ? process.env.VITE_YEOLLIN_BASE_PATH : undefined,
  deploymentId: isPagesDemo ? STATIC_EXPORT_DEPLOYMENT_ID : undefined,
  output: 'export',
}

export default nextConfig
