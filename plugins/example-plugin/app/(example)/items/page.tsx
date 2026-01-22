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
      <Flex justifyContent="space-between" alignItems="center" mb={4}>
        <Text typography="heading">Items</Text>
      </Flex>

      <Flex flexDirection="column" gap={3}>
        {items.map((item) => (
          <Box
            key={item.id}
            bg="$background"
            p={4}
            borderRadius="8px"
            border="1px solid $border"
            _hover={{ borderColor: '$primary' }}
          >
            <Flex justifyContent="space-between" alignItems="flex-start">
              <Box>
                <Text typography="subheading" mb={1}>
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
