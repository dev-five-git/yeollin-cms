'use client'

import { Box, Flex, Text } from '@devup-ui/react'
import { useRouter } from 'next/navigation'

import { resetMockApi } from '@/lib/mock-api'

const DEMO_MODE = import.meta.env.VITE_YEOLLIN_DEMO === 'true'

/**
 * Header component with logout functionality.
 * Requires 'use client' due to cookie manipulation and useRouter.
 */
export function Header() {
  const router = useRouter()

  const handleReset = () => {
    resetMockApi()
    window.location.reload()
  }

  const handleLogout = () => {
    // Clear auth cookies
    document.cookie = 'access_token=; path=/; max-age=0'
    document.cookie = 'refresh_token=; path=/; max-age=0'

    // Redirect to signin
    router.push('/signin')
  }

  return (
    <Box
      alignItems="center"
      bg="$background"
      borderBottom="1px solid $border"
      display="flex"
      h="56px"
      justifyContent="space-between"
      px={6}
      w="100%"
    >
      {DEMO_MODE ? (
        <Flex alignItems="center" gap={2}>
          <Box bg="$primaryLight" borderRadius="999px" px={3} py={1}>
            <Text color="$primary" typography="label">
              Interactive demo
            </Text>
          </Box>
          <Text
            color="$textTertiary"
            display={['none', null, 'block']}
            typography="label"
          >
            Data stays in this browser tab
          </Text>
        </Flex>
      ) : (
        <Box />
      )}
      <Flex
        _hover={{ bg: '$backgroundSecondary', color: '$text' }}
        alignItems="center"
        borderRadius="6px"
        color="$textSecondary"
        cursor="pointer"
        onClick={DEMO_MODE ? handleReset : handleLogout}
        px={3}
        py={2}
      >
        <Text typography="label">{DEMO_MODE ? 'Reset demo' : 'Logout'}</Text>
      </Flex>
    </Box>
  )
}
