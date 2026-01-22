'use client'

import { Box, Flex, Text } from '@devup-ui/react'

export default function ExampleDashboard() {
  return (
    <Box p={6}>
      <Text typography="heading" mb={4}>
        Example Plugin
      </Text>
      <Flex gap={4} flexDirection="column">
        <Box
          bg="$background"
          p={4}
          borderRadius="8px"
          border="1px solid $border"
        >
          <Text typography="subheading" mb={2}>
            Welcome
          </Text>
          <Text color="$textSecondary">
            This is an example plugin demonstrating Yeollin CMS plugin
            architecture.
          </Text>
        </Box>
      </Flex>
    </Box>
  )
}
