'use client'

import { Box, Flex, Text } from '@devup-ui/react'

interface DashboardStats {
  label: string
  value: string | number
}

export default function DashboardPage() {
  const stats: DashboardStats[] = [
    { label: 'Total Content', value: 0 },
    { label: 'Published', value: 0 },
    { label: 'Drafts', value: 0 },
  ]

  return (
    <Box p={6}>
      <Text mb={4} typography="heading">
        Dashboard
      </Text>
      <Flex flexWrap="wrap" gap={4}>
        {stats.map((stat) => (
          <Box
            key={stat.label}
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            flex={1}
            minW="200px"
            p={6}
          >
            <Text color="$textSecondary" mb={2} typography="label">
              {stat.label}
            </Text>
            <Text typography="heading">{stat.value}</Text>
          </Box>
        ))}
      </Flex>
    </Box>
  )
}
