import { Box, Flex } from '@devup-ui/react'

import { Header } from '@/components/layout/Header'
import { Sidebar } from '@/components/layout/Sidebar'

export default function AuthLayout({
  children,
}: {
  children: React.ReactNode
}) {
  // TODO: Add authentication check here
  return (
    <Flex h="100vh">
      <Sidebar />
      <Flex flex={1} flexDirection="column" overflow="hidden">
        <Header />
        <Box flex={1} overflow="auto" bg="$backgroundSecondary">
          {children}
        </Box>
      </Flex>
    </Flex>
  )
}
