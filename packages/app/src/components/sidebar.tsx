'use client'

import { Box, Flex, Text } from '@devup-ui/react'
import Link from 'next/link'
import { usePathname } from 'next/navigation'

interface MenuItem {
  id: string
  label: string
  icon?: string
  path: string
  children?: MenuItem[]
}

// This will be dynamically loaded from API in production
const menuItems: MenuItem[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    icon: 'home',
    path: '/',
  },
  {
    id: 'example',
    label: 'Example',
    icon: 'box',
    path: '/example',
    children: [
      { id: 'example-items', label: 'Items', path: '/example/items' },
      { id: 'example-settings', label: 'Settings', path: '/example/settings' },
    ],
  },
]

export function Sidebar() {
  const pathname = usePathname()

  return (
    <Box
      w="260px"
      h="100vh"
      bg="$background"
      borderRight="1px solid $border"
      p={4}
    >
      {/* Logo */}
      <Flex alignItems="center" gap={2} mb={6} px={2}>
        <Box
          w="32px"
          h="32px"
          bg="$primary"
          borderRadius="8px"
          display="flex"
          alignItems="center"
          justifyContent="center"
        >
          <Text color="white" fontWeight={700}>
            Y
          </Text>
        </Box>
        <Text typography="subheading">Yeollin CMS</Text>
      </Flex>

      {/* Navigation */}
      <Flex flexDirection="column" gap={1}>
        {menuItems.map((item) => (
          <NavItem key={item.id} item={item} pathname={pathname} />
        ))}
      </Flex>
    </Box>
  )
}

function NavItem({ item, pathname }: { item: MenuItem; pathname: string }) {
  const isActive = pathname === item.path
  const hasChildren = item.children && item.children.length > 0
  const isChildActive = item.children?.some((child) => pathname === child.path)

  return (
    <Box>
      <Link href={item.path} style={{ textDecoration: 'none' }}>
        <Flex
          alignItems="center"
          gap={3}
          px={3}
          py={2}
          borderRadius="8px"
          bg={isActive ? '$primary' : 'transparent'}
          color={isActive ? 'white' : '$text'}
          _hover={{ bg: isActive ? '$primary' : '$backgroundSecondary' }}
        >
          <Text fontSize="14px">{item.label}</Text>
        </Flex>
      </Link>

      {/* Children */}
      {hasChildren && (isActive || isChildActive) && (
        <Flex flexDirection="column" gap={1} mt={1} ml={4}>
          {item.children!.map((child) => (
            <Link
              key={child.id}
              href={child.path}
              style={{ textDecoration: 'none' }}
            >
              <Flex
                alignItems="center"
                px={3}
                py={2}
                borderRadius="8px"
                bg={
                  pathname === child.path
                    ? '$backgroundSecondary'
                    : 'transparent'
                }
                color={pathname === child.path ? '$primary' : '$textSecondary'}
                _hover={{ bg: '$backgroundSecondary' }}
              >
                <Text fontSize="13px">{child.label}</Text>
              </Flex>
            </Link>
          ))}
        </Flex>
      )}
    </Box>
  )
}
