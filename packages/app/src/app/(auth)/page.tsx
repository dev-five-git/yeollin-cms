import { Box, Flex, Text } from '@devup-ui/react'

export default function HomePage() {
  return (
    <Box p={6}>
      <Text mb={4} typography="heading">
        Dashboard
      </Text>
      <Flex flexWrap="wrap" gap={4}>
        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          flex={1}
          minW="200px"
          p={6}
        >
          <Text color="$textSecondary" mb={2} typography="label">
            Total Content
          </Text>
          <Text typography="heading">0</Text>
        </Box>
        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          flex={1}
          minW="200px"
          p={6}
        >
          <Text color="$textSecondary" mb={2} typography="label">
            Published
          </Text>
          <Text typography="heading">0</Text>
        </Box>
        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          flex={1}
          minW="200px"
          p={6}
        >
          <Text color="$textSecondary" mb={2} typography="label">
            Drafts
          </Text>
          <Text typography="heading">0</Text>
        </Box>
      </Flex>
    </Box>
  )
}
