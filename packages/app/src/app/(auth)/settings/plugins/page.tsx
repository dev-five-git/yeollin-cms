import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { readFile } from 'fs/promises'
import Link from 'next/link'
import path from 'path'

interface PluginInfo {
  name: string
  version: string
  author: string | null
  description: string | null
  license: string | null
}

async function getPlugins(): Promise<PluginInfo[]> {
  try {
    const pluginsPath = path.join(process.cwd(), 'src', 'plugins.json')
    const content = await readFile(pluginsPath, 'utf-8')
    return JSON.parse(content)
  } catch {
    return []
  }
}

interface PluginCardProps {
  plugin: PluginInfo
}

function PluginCard({ plugin }: PluginCardProps) {
  return (
    <Box
      _hover={{ borderColor: '$primary' }}
      bg="$background"
      border="1px solid $border"
      borderRadius="12px"
      p={5}
      transition="all 0.2s ease"
    >
      <Flex alignItems="flex-start" justifyContent="space-between" mb={3}>
        <Flex alignItems="center" gap={3}>
          <Box
            alignItems="center"
            bg="$primaryLight"
            borderRadius="10px"
            display="flex"
            fontSize="24px"
            h="48px"
            justifyContent="center"
            w="48px"
          >
            🧩
          </Box>
          <VStack alignItems="flex-start" gap={1}>
            <Text color="$text" typography="subheading">
              {plugin.name}
            </Text>
            {plugin.author ? (
              <Text color="$textSecondary" typography="label">
                by {plugin.author}
              </Text>
            ) : null}
          </VStack>
        </Flex>
        <Box bg="$backgroundSecondary" borderRadius="6px" px={2} py={1}>
          <Text color="$textSecondary" typography="label">
            v{plugin.version}
          </Text>
        </Box>
      </Flex>
      {plugin.description ? (
        <Text color="$textSecondary" typography="body">
          {plugin.description}
        </Text>
      ) : (
        <Text color="$textTertiary" fontStyle="italic" typography="body">
          No description provided
        </Text>
      )}
    </Box>
  )
}

export default async function PluginsPage() {
  const plugins = await getPlugins()

  return (
    <Box p={6}>
      <VStack alignItems="flex-start" gap={6}>
        <Flex alignItems="center" gap={4}>
          <Link href="/settings" style={{ textDecoration: 'none' }}>
            <Text
              _hover={{ color: '$primary' }}
              color="$textSecondary"
              cursor="pointer"
              typography="body"
            >
              Settings
            </Text>
          </Link>
          <Text color="$textTertiary">/</Text>
          <Text color="$text" typography="body">
            Plugins
          </Text>
        </Flex>

        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">Installed Plugins</Text>
          <Text color="$textSecondary" typography="body">
            {plugins.length} plugin{plugins.length !== 1 ? 's' : ''} installed
          </Text>
        </VStack>

        {plugins.length === 0 ? (
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={8}
            textAlign="center"
            w="100%"
          >
            <Text color="$textSecondary" mb={2} typography="subheading">
              No plugins installed
            </Text>
            <Text color="$textTertiary" typography="body">
              Plugins extend the functionality of your CMS
            </Text>
          </Box>
        ) : (
          <Grid columns={['1fr', null, '1fr 1fr']} gap={4} w="100%">
            {plugins.map((plugin) => (
              <PluginCard key={plugin.name} plugin={plugin} />
            ))}
          </Grid>
        )}
      </VStack>
    </Box>
  )
}
