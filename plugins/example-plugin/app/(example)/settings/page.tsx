'use client'

import { Box, Flex, Text } from '@devup-ui/react'

export default function SettingsPage() {
  return (
    <Box p={6}>
      <Text mb={4} typography="heading">
        Settings
      </Text>
      <Box bg="$background" border="1px solid $border" borderRadius="8px" p={4}>
        <Flex flexDirection="column" gap={4}>
          <Box>
            <Text mb={1} typography="label">
              Plugin Name
            </Text>
            <Text color="$textSecondary">example-plugin</Text>
          </Box>
          <Box>
            <Text mb={1} typography="label">
              Version
            </Text>
            <Text color="$textSecondary">0.1.0</Text>
          </Box>
          <Box>
            <Text mb={1} typography="label">
              Status
            </Text>
            <Flex alignItems="center" gap={2}>
              <Box bg="$success" borderRadius="50%" h="8px" w="8px" />
              <Text color="$success">Active</Text>
            </Flex>
          </Box>
        </Flex>
      </Box>
    </Box>
  )
}
