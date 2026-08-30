import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { readFile } from 'fs/promises'
import Link from 'next/link'
import path from 'path'

interface PluginInfo {
  description?: string
  name: string
  settings?: {
    customPage: boolean
    pagePath: string
  }
}

async function getPlugins(): Promise<PluginInfo[]> {
  try {
    const content = await readFile(
      path.join(process.cwd(), 'src', 'plugins.json'),
      'utf-8',
    )
    return JSON.parse(content)
  } catch {
    return []
  }
}

export default async function SettingsPage() {
  const configurablePlugins = (await getPlugins()).filter(
    (plugin) => plugin.settings,
  )

  return (
    <Box p={6}>
      <VStack alignItems="flex-start" gap={6}>
        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">Settings</Text>
          <Text color="$textSecondary" typography="body">
            Configure installed plugins from their typed settings contracts.
          </Text>
        </VStack>

        <Grid columns={['1fr', null, '1fr 1fr']} gap={4} w="100%">
          {configurablePlugins.map((plugin) => (
            <Link
              key={plugin.name}
              href={plugin.settings!.pagePath}
              style={{ textDecoration: 'none' }}
            >
              <Box
                _hover={{
                  borderColor: '$primary',
                  transform: 'translateY(-2px)',
                }}
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                h="100%"
                p={6}
                transition="all 0.2s ease"
              >
                <Flex alignItems="center" justifyContent="space-between" mb={3}>
                  <Text color="$text" typography="subheading">
                    {plugin.name}
                  </Text>
                  {plugin.settings?.customPage ? (
                    <Text color="$textSecondary" typography="label">
                      Custom
                    </Text>
                  ) : null}
                </Flex>
                <Text color="$textSecondary" typography="body">
                  {plugin.description ?? 'Configure plugin settings'}
                </Text>
              </Box>
            </Link>
          ))}

          <Link href="/settings/plugins" style={{ textDecoration: 'none' }}>
            <Box
              _hover={{
                borderColor: '$primary',
                transform: 'translateY(-2px)',
              }}
              bg="$background"
              border="1px solid $border"
              borderRadius="12px"
              h="100%"
              p={6}
              transition="all 0.2s ease"
            >
              <Text color="$text" mb={3} typography="subheading">
                Installed plugins
              </Text>
              <Text color="$textSecondary" typography="body">
                View versions, authors, and plugin metadata.
              </Text>
            </Box>
          </Link>
        </Grid>
      </VStack>
    </Box>
  )
}
