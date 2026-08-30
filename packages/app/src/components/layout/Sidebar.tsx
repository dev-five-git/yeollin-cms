import { Box, Flex, Text } from '@devup-ui/react'
import { readFile } from 'fs/promises'
import path from 'path'

import { NavItem } from './NavItem'

interface MenuItem {
  id: string
  label: string
  path: string
  children?: MenuItem[]
}

/** System menu items (always present) */
const systemMenuItems: MenuItem[] = [
  {
    id: 'settings',
    label: 'Settings',
    path: '/settings',
    children: [
      { id: 'settings-plugins', label: 'Plugins', path: '/settings/plugins' },
    ],
  },
]

/**
 * Sidebar component that reads menu items from generated menus.json at build time.
 * Server Component - no 'use client' needed.
 */
export async function Sidebar() {
  // Read menus.json at build time (SSG)
  const menusPath = path.join(process.cwd(), 'src', 'menus.json')
  const menusContent = await readFile(menusPath, 'utf-8')
  const pluginMenuItems: MenuItem[] = JSON.parse(menusContent)

  return (
    <Box
      bg="$background"
      borderBottom={['1px solid $border', null, 'none']}
      borderRight={['none', null, '1px solid $border']}
      display="flex"
      flexDirection={['row', null, 'column']}
      flexShrink={0}
      h={['auto', null, '100vh']}
      overflowX={['auto', null, 'visible']}
      overflowY="hidden"
      p={[2, null, 4]}
      w={['100%', null, '260px']}
    >
      {/* Logo */}
      <Flex
        alignItems="center"
        display={['none', null, 'flex']}
        gap={2}
        mb={6}
        px={2}
      >
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

      {/* Plugin Navigation */}
      <Flex flex={[0, null, 1]} flexDirection={['row', null, 'column']} gap={1}>
        {pluginMenuItems.map((item) => (
          <NavItem key={item.id} item={item} />
        ))}
      </Flex>

      {/* System Navigation */}
      <Box
        borderLeft={['1px solid $border', null, 'none']}
        borderTop={['none', null, '1px solid $border']}
        ml={[2, null, 0]}
        mt={[0, null, 4]}
        pl={[2, null, 0]}
        pt={[0, null, 4]}
      >
        <Text
          color="$textTertiary"
          display={['none', null, 'block']}
          mb={2}
          px={2}
          typography="label"
        >
          System
        </Text>
        <Flex flexDirection={['row', null, 'column']} gap={1}>
          {systemMenuItems.map((item) => (
            <NavItem key={item.id} item={item} />
          ))}
        </Flex>
      </Box>
    </Box>
  )
}
