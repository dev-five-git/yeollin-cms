import Link from 'next/link'
import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'

interface SettingsCardProps {
  title: string
  description: string
  href: string
  icon: string
}

function SettingsCard({ title, description, href, icon }: SettingsCardProps) {
  return (
    <Link href={href} style={{ textDecoration: 'none' }}>
      <Box
        bg="$background"
        p={6}
        borderRadius="12px"
        border="1px solid $border"
        transition="all 0.2s ease"
        _hover={{ borderColor: '$primary', transform: 'translateY(-2px)' }}
        cursor="pointer"
        h="100%"
      >
        <Flex alignItems="center" gap={4} mb={3}>
          <Box
            w="40px"
            h="40px"
            borderRadius="8px"
            bg="$primaryLight"
            display="flex"
            alignItems="center"
            justifyContent="center"
            fontSize="20px"
          >
            {icon}
          </Box>
          <Text typography="subheading" color="$text">
            {title}
          </Text>
        </Flex>
        <Text typography="body" color="$textSecondary">
          {description}
        </Text>
      </Box>
    </Link>
  )
}

export default function SettingsPage() {
  return (
    <Box p={6}>
      <VStack alignItems="flex-start" gap={6}>
        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">Settings</Text>
          <Text typography="body" color="$textSecondary">
            Manage your CMS configuration and preferences
          </Text>
        </VStack>

        <Grid
          columns={['1fr', '1fr 1fr', '1fr 1fr 1fr']}
          gap={4}
          w="100%"
        >
          <SettingsCard
            title="Plugins"
            description="View and manage installed plugins"
            href="/settings/plugins"
            icon="🧩"
          />
          <SettingsCard
            title="General"
            description="Basic CMS settings and preferences"
            href="/settings/general"
            icon="⚙️"
          />
          <SettingsCard
            title="Users"
            description="Manage users and permissions"
            href="/settings/users"
            icon="👥"
          />
        </Grid>
      </VStack>
    </Box>
  )
}
