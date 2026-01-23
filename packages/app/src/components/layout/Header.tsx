'use client'

import { useRouter } from 'next/navigation'
import { Box, Flex, Text } from '@devup-ui/react'

/**
 * Header component with logout functionality.
 * Requires 'use client' due to cookie manipulation and useRouter.
 */
export function Header() {
  const router = useRouter()

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
      justifyContent="flex-end"
      px={6}
      w="100%"
    >
      <Flex
        alignItems="center"
        borderRadius="6px"
        color="$textSecondary"
        cursor="pointer"
        onClick={handleLogout}
        px={3}
        py={2}
        _hover={{ bg: '$backgroundSecondary', color: '$text' }}
      >
        <Text typography="label">Logout</Text>
      </Flex>
    </Box>
  )
}
