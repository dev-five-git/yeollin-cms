'use client'

import { Box, Flex, Text } from '@devup-ui/react'

export default function SettingsPage() {
  return (
    <Box p={6}>
      <Text typography="heading" mb={4}>
        Settings
      </Text>
      <Box bg="$background" p={4} borderRadius="8px" border="1px solid $border">
        <Flex flexDirection="column" gap={4}>
          <Box>
            <Text typography="label" mb={1}>
              Plugin Name
            </Text>
            <Text color="$textSecondary">example-plugin</Text>
          </Box>
          <Box>
            <Text typography="label" mb={1}>
              Version
            </Text>
            <Text color="$textSecondary">0.1.0</Text>
          </Box>
          <Box>
            <Text typography="label" mb={1}>
              Status
            </Text>
            <Flex alignItems="center" gap={2}>
              <Box w="8px" h="8px" borderRadius="50%" bg="$success" />
              <Text color="$success">Active</Text>
            </Flex>
          </Box>
        </Flex>
      </Box>
    </Box>
  )
}
