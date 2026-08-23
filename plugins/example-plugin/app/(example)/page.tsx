'use client'

import { Box, Flex, Text } from '@devup-ui/react'

export default function ExampleDashboard() {
  return (
    <Box p={6}>
      <Text mb={4} typography="heading">
        Example Plugin
      </Text>
      <Flex flexDirection="column" gap={4}>
        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="8px"
          p={4}
        >
          <Text mb={2} typography="subheading">
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
