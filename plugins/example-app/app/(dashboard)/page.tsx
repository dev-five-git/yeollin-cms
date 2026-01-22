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
      <Text typography="heading" mb={4}>
        Dashboard
      </Text>
      <Flex gap={4} flexWrap="wrap">
        {stats.map((stat) => (
          <Box
            key={stat.label}
            bg="$background"
            p={6}
            borderRadius="12px"
            border="1px solid $border"
            minW="200px"
            flex={1}
          >
            <Text typography="label" color="$textSecondary" mb={2}>
              {stat.label}
            </Text>
            <Text typography="heading">{stat.value}</Text>
          </Box>
        ))}
      </Flex>
    </Box>
  )
}
