import { Box, Grid, Text, VStack } from '@devup-ui/react'
import Link from 'next/link'

interface ContentCollectionLink {
  label: string
  path: string
}

interface ContentCollectionsHubProps {
  collections: ContentCollectionLink[]
  pluginName: string
}

function humanize(value: string) {
  return value
    .split(/[-_]/)
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join(' ')
}

export function ContentCollectionsHub({
  collections,
  pluginName,
}: ContentCollectionsHubProps) {
  return (
    <Box p={[4, null, 8]}>
      <VStack alignItems="stretch" gap={6} maxW="960px">
        <VStack alignItems="flex-start" gap={1}>
          <Text typography="heading">{humanize(pluginName)}</Text>
          <Text color="$textSecondary" typography="body">
            Choose a typed collection to manage its drafts and published
            entries.
          </Text>
        </VStack>
        <Grid
          gap={4}
          gridTemplateColumns={['1fr', 'repeat(2, minmax(0, 1fr))']}
        >
          {collections.map((collection) => (
            <Link
              key={collection.path}
              href={collection.path}
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
                p={5}
                transition="border-color 120ms ease, transform 120ms ease"
              >
                <VStack alignItems="flex-start" gap={1}>
                  <Text typography="subheading">{collection.label}</Text>
                  <Text color="$textSecondary" typography="body">
                    Create, edit, publish, and unpublish entries.
                  </Text>
                </VStack>
              </Box>
            </Link>
          ))}
        </Grid>
      </VStack>
    </Box>
  )
}
