import { readFile } from 'fs/promises'
import path from 'path'
import { Box, Flex, Text } from '@devup-ui/react'

import { NavItem } from './NavItem'

interface MenuItem {
  id: string
  label: string
  path: string
  children?: MenuItem[]
}

/**
 * Sidebar component that reads menu items from generated menus.json at build time.
 * Server Component - no 'use client' needed.
 */
export async function Sidebar() {
  // Read menus.json at build time (SSG)
  const menusPath = path.join(process.cwd(), 'src', 'menus.json')
  const menusContent = await readFile(menusPath, 'utf-8')
  const menuItems: MenuItem[] = JSON.parse(menusContent)

  return (
    <Box
      bg="$background"
      borderRight="1px solid $border"
      h="100vh"
      p={4}
      w="260px"
    >
      {/* Logo */}
      <Flex alignItems="center" gap={2} mb={6} px={2}>
        <Box
          alignItems="center"
          bg="$primary"
          borderRadius="8px"
          display="flex"
          h="32px"
          justifyContent="center"
          w="32px"
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
          <NavItem key={item.id} item={item} />
        ))}
      </Flex>
    </Box>
  )
}
