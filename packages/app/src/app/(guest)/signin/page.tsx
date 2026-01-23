'use client'

import { useState } from 'react'
import { useRouter } from 'next/navigation'
import { Box, Flex, Text, VStack } from '@devup-ui/react'

export default function SignInPage() {
  const router = useRouter()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError('')
    setLoading(true)

    try {
      const response = await fetch('/api/auth/login', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ username, password }),
      })

      if (!response.ok) {
        const data = await response.json()
        throw new Error(data.error || 'Login failed')
      }

      const data = await response.json()

      // Store tokens in cookies
      document.cookie = `access_token=${data.access_token}; path=/; max-age=${data.expires_in}`
      document.cookie = `refresh_token=${data.refresh_token}; path=/; max-age=${60 * 60 * 24 * 7}` // 7 days

      // Redirect to dashboard
      router.push('/')
      router.refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred')
    } finally {
      setLoading(false)
    }
  }

  return (
    <Flex
      h="100vh"
      alignItems="center"
      justifyContent="center"
      bg="$backgroundSecondary"
    >
      <Box
        bg="$background"
        p={8}
        borderRadius="16px"
        border="1px solid $border"
        w="100%"
        maxW="400px"
        boxShadow="0 4px 24px rgba(0, 0, 0, 0.1)"
      >
        <VStack gap={6} alignItems="stretch">
          <VStack gap={2} alignItems="center">
            <Text typography="heading" textAlign="center">
              Yeollin CMS
            </Text>
            <Text typography="body" color="$textSecondary" textAlign="center">
              Sign in to your account
            </Text>
          </VStack>

          <form onSubmit={handleSubmit}>
            <VStack gap={4} alignItems="stretch">
              {error && (
                <Box
                  bg="$errorLight"
                  p={3}
                  borderRadius="8px"
                  border="1px solid $error"
                >
                  <Text typography="body" color="$error">
                    {error}
                  </Text>
                </Box>
              )}

              <VStack gap={2} alignItems="stretch">
                <Text as="label" typography="label" htmlFor="username">
                  Username
                </Text>
                <Box
                  as="input"
                  id="username"
                  type="text"
                  value={username}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setUsername(e.target.value)
                  }
                  required
                  p={3}
                  borderRadius="8px"
                  border="1px solid $border"
                  bg="$background"
                  color="$text"
                  fontSize="16px"
                  outline="none"
                  _focus={{ borderColor: '$primary' }}
                  _placeholder={{ color: '$textTertiary' }}
                  placeholder="Enter username"
                />
              </VStack>

              <VStack gap={2} alignItems="stretch">
                <Text as="label" typography="label" htmlFor="password">
                  Password
                </Text>
                <Box
                  as="input"
                  id="password"
                  type="password"
                  value={password}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setPassword(e.target.value)
                  }
                  required
                  p={3}
                  borderRadius="8px"
                  border="1px solid $border"
                  bg="$background"
                  color="$text"
                  fontSize="16px"
                  outline="none"
                  _focus={{ borderColor: '$primary' }}
                  _placeholder={{ color: '$textTertiary' }}
                  placeholder="Enter password"
                />
              </VStack>

              <Box
                as="button"
                type="submit"
                disabled={loading}
                p={3}
                borderRadius="8px"
                border="none"
                bg="$primary"
                color="white"
                fontSize="16px"
                fontWeight="600"
                cursor={loading ? 'not-allowed' : 'pointer'}
                opacity={loading ? 0.7 : 1}
                transition="all 0.2s ease"
                _hover={{ bg: '$primaryHover' }}
                _active={{ transform: 'scale(0.98)' }}
              >
                {loading ? 'Signing in...' : 'Sign in'}
              </Box>
            </VStack>
          </form>

          <Text typography="label" color="$textTertiary" textAlign="center">
            Use superadmin credentials to sign in
          </Text>
        </VStack>
      </Box>
    </Flex>
  )
}
