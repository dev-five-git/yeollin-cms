'use client'

import { Box, Flex, Text } from '@devup-ui/react'
import Link from 'next/link'
import { usePathname } from 'next/navigation'

interface MenuItem {
  id: string
  label: string
  path: string
  children?: MenuItem[]
}

interface NavItemProps {
  item: MenuItem
}

/**
 * Navigation item component with active state detection.
 * Requires 'use client' due to usePathname hook.
 */
export function NavItem({ item }: NavItemProps) {
  const pathname = usePathname()
  const isActive = pathname === item.path
  const hasChildren = item.children && item.children.length > 0
  const isChildActive = item.children?.some((child) => pathname === child.path)

  return (
    <Box flexShrink={0}>
      <Link href={item.path} style={{ textDecoration: 'none' }}>
        <Flex
          _hover={{ bg: isActive ? '$primary' : '$backgroundSecondary' }}
          alignItems="center"
          bg={isActive ? '$primary' : 'transparent'}
          borderRadius="8px"
          color={isActive ? 'white' : '$text'}
          gap={3}
          px={3}
          py={2}
          whiteSpace="nowrap"
        >
          <Text fontSize="14px">{item.label}</Text>
        </Flex>
      </Link>

      {hasChildren && (isActive || isChildActive) && (
        <Flex flexDirection="column" gap={1} ml={[0, null, 4]} mt={1}>
          {item.children!.map((child) => (
            <Link
              key={child.id}
              href={child.path}
              style={{ textDecoration: 'none' }}
            >
              <Flex
                _hover={{ bg: '$backgroundSecondary' }}
                alignItems="center"
                bg={
                  pathname === child.path
                    ? '$backgroundSecondary'
                    : 'transparent'
                }
                borderRadius="8px"
                color={pathname === child.path ? '$primary' : '$textSecondary'}
                px={3}
                py={2}
                whiteSpace="nowrap"
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
