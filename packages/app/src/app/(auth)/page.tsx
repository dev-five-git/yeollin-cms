import { Box, Flex, Text } from '@devup-ui/react'

export default function HomePage() {
  return (
    <Box p={6}>
      <Text typography="heading" mb={4}>
        Dashboard
      </Text>
      <Flex gap={4} flexWrap="wrap">
        <Box
          bg="$background"
          p={6}
          borderRadius="12px"
          border="1px solid $border"
          minW="200px"
          flex={1}
        >
          <Text typography="label" color="$textSecondary" mb={2}>
            Total Content
          </Text>
          <Text typography="heading">0</Text>
        </Box>
        <Box
          bg="$background"
          p={6}
          borderRadius="12px"
          border="1px solid $border"
          minW="200px"
          flex={1}
        >
          <Text typography="label" color="$textSecondary" mb={2}>
            Published
          </Text>
          <Text typography="heading">0</Text>
        </Box>
        <Box
          bg="$background"
          p={6}
          borderRadius="12px"
          border="1px solid $border"
          minW="200px"
          flex={1}
        >
          <Text typography="label" color="$textSecondary" mb={2}>
            Drafts
          </Text>
          <Text typography="heading">0</Text>
        </Box>
      </Flex>
    </Box>
  )
}
