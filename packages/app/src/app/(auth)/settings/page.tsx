import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import Link from 'next/link'

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
        _hover={{ borderColor: '$primary', transform: 'translateY(-2px)' }}
        bg="$background"
        border="1px solid $border"
        borderRadius="12px"
        cursor="pointer"
        h="100%"
        p={6}
        transition="all 0.2s ease"
      >
        <Flex alignItems="center" gap={4} mb={3}>
          <Box
            alignItems="center"
            bg="$primaryLight"
            borderRadius="8px"
            display="flex"
            fontSize="20px"
            h="40px"
            justifyContent="center"
            w="40px"
          >
            {icon}
          </Box>
          <Text color="$text" typography="subheading">
            {title}
          </Text>
        </Flex>
        <Text color="$textSecondary" typography="body">
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
          <Text color="$textSecondary" typography="body">
            Manage your CMS configuration and preferences
          </Text>
        </VStack>

        <Grid columns={['1fr', '1fr 1fr', '1fr 1fr 1fr']} gap={4} w="100%">
          <SettingsCard
            description="View and manage installed plugins"
            href="/settings/plugins"
            icon="🧩"
            title="Plugins"
          />
          <SettingsCard
            description="Basic CMS settings and preferences"
            href="/settings/general"
            icon="⚙️"
            title="General"
          />
          <SettingsCard
            description="Manage users and permissions"
            href="/settings/users"
            icon="👥"
            title="Users"
          />
        </Grid>
      </VStack>
    </Box>
  )
}
