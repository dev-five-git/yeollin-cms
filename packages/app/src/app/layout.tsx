import { ThemeScript } from '@devup-ui/react'
import { resetCss } from '@devup-ui/reset-css'
import type { Metadata } from 'next'

import { Providers } from '@/components/providers'
import {
  installStaticExportRscTransport,
  STATIC_EXPORT_DEPLOYMENT_ID,
} from '@/lib/static-export-rsc-transport'

function escapeScriptClosingTag(script: string): string {
  return script.replace(/<\/script/giu, '\\u003c/script')
}

const staticExportRscTransport = escapeScriptClosingTag(
  `(${installStaticExportRscTransport.toString()})(${JSON.stringify(STATIC_EXPORT_DEPLOYMENT_ID)}, ${JSON.stringify(process.env.VITE_YEOLLIN_BASE_PATH ?? '')})`,
)

const needsStaticExportRscTransport =
  process.env.NODE_ENV === 'production' &&
  process.env.VITE_YEOLLIN_DEMO === 'true'

export const metadata: Metadata = {
  title: 'Yeollin CMS',
  description: 'Open, extensible CMS framework',
}

resetCss()

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <head>
        {needsStaticExportRscTransport && (
          <script data-vinext-static-rsc-transport="">
            {staticExportRscTransport}
          </script>
        )}
        <ThemeScript />
      </head>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
