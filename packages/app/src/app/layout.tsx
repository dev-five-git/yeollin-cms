import { ThemeScript } from '@devup-ui/react'
import { Box, Flex } from '@devup-ui/react'
import type { Metadata } from 'next'

import { Providers } from '@/components/providers'
import { Sidebar } from '@/components/sidebar'

export const metadata: Metadata = {
  title: 'Yeollin CMS',
  description: 'Open, extensible CMS framework',
}

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
        <Providers>
          <Flex h="100vh">
            <Sidebar />
            <Box flex={1} overflow="auto" bg="$backgroundSecondary">
              {children}
            </Box>
          </Flex>
        </Providers>
      </body>
    </html>
  )
}
