import { Box, Flex } from '@devup-ui/react'

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
      <Box flex={1} overflow="auto" bg="$backgroundSecondary">
        {children}
      </Box>
    </Flex>
  )
}
