import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import Link from 'next/link'
import { readFile } from 'fs/promises'
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
      bg="$background"
      p={5}
      borderRadius="12px"
      border="1px solid $border"
      transition="all 0.2s ease"
      _hover={{ borderColor: '$primary' }}
    >
      <Flex justifyContent="space-between" alignItems="flex-start" mb={3}>
        <Flex alignItems="center" gap={3}>
          <Box
            w="48px"
            h="48px"
            borderRadius="10px"
            bg="$primaryLight"
            display="flex"
            alignItems="center"
            justifyContent="center"
            fontSize="24px"
          >
            🧩
          </Box>
          <VStack alignItems="flex-start" gap={1}>
            <Text typography="subheading" color="$text">
              {plugin.name}
            </Text>
            {plugin.author ? (
              <Text typography="label" color="$textSecondary">
                by {plugin.author}
              </Text>
            ) : null}
          </VStack>
        </Flex>
        <Box
          bg="$backgroundSecondary"
          px={2}
          py={1}
          borderRadius="6px"
        >
          <Text typography="label" color="$textSecondary">
            v{plugin.version}
          </Text>
        </Box>
      </Flex>
      {plugin.description ? (
        <Text typography="body" color="$textSecondary">
          {plugin.description}
        </Text>
      ) : (
        <Text typography="body" color="$textTertiary" fontStyle="italic">
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
              typography="body"
              color="$textSecondary"
              _hover={{ color: '$primary' }}
              cursor="pointer"
            >
              Settings
            </Text>
          </Link>
          <Text color="$textTertiary">/</Text>
          <Text typography="body" color="$text">
            Plugins
          </Text>
        </Flex>

        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">Installed Plugins</Text>
          <Text typography="body" color="$textSecondary">
            {plugins.length} plugin{plugins.length !== 1 ? 's' : ''} installed
          </Text>
        </VStack>

        {plugins.length === 0 ? (
          <Box
            bg="$background"
            p={8}
            borderRadius="12px"
            border="1px solid $border"
            w="100%"
            textAlign="center"
          >
            <Text typography="subheading" color="$textSecondary" mb={2}>
              No plugins installed
            </Text>
            <Text typography="body" color="$textTertiary">
              Plugins extend the functionality of your CMS
            </Text>
          </Box>
        ) : (
          <Grid
            columns={['1fr', '1fr', '1fr 1fr']}
            gap={4}
            w="100%"
          >
            {plugins.map((plugin) => (
              <PluginCard key={plugin.name} plugin={plugin} />
            ))}
          </Grid>
        )}
      </VStack>
    </Box>
  )
}
