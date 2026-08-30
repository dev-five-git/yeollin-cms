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
    <Flex flexDirection={['column', null, 'row']} h="100vh">
      <Sidebar />
      <Flex flex={1} flexDirection="column" minH={0} minW={0} overflow="hidden">
        <Header />
        <Box bg="$backgroundSecondary" flex={1} overflow="auto">
          {children}
        </Box>
      </Flex>
    </Flex>
  )
}
