import { ThemeScript } from '@devup-ui/react'
import { resetCss } from '@devup-ui/reset-css'
import type { Metadata } from 'next'

import { Providers } from '@/components/providers'

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
        <ThemeScript />
      </head>
      <body>
        <Providers>{children}</Providers>
      </body>
    </html>
  )
}
