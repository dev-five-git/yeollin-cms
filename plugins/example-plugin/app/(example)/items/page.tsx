'use client'

import { Box, Flex, Text } from '@devup-ui/react'

interface ExampleItem {
  id: string
  name: string
  description?: string
  createdAt: string
}

export default function ItemsPage() {
  const items: ExampleItem[] = [
    {
      id: '1',
      name: 'First Item',
      description: 'This is the first example item',
      createdAt: '2024-01-01T00:00:00Z',
    },
    {
      id: '2',
      name: 'Second Item',
      createdAt: '2024-01-02T00:00:00Z',
    },
  ]

  return (
    <Box p={6}>
      <Flex alignItems="center" justifyContent="space-between" mb={4}>
        <Text typography="heading">Items</Text>
      </Flex>

      <Flex flexDirection="column" gap={3}>
        {items.map((item) => (
          <Box
            key={item.id}
            _hover={{ borderColor: '$primary' }}
            bg="$background"
            border="1px solid $border"
            borderRadius="8px"
            p={4}
          >
            <Flex alignItems="flex-start" justifyContent="space-between">
              <Box>
                <Text mb={1} typography="subheading">
                  {item.name}
                </Text>
                {item.description && (
                  <Text color="$textSecondary" fontSize="14px">
                    {item.description}
                  </Text>
                )}
              </Box>
              <Text color="$textTertiary" fontSize="12px">
                {new Date(item.createdAt).toLocaleDateString()}
              </Text>
            </Flex>
          </Box>
        ))}
      </Flex>
    </Box>
  )
}
