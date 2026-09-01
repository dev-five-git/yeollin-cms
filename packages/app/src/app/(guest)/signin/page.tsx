'use client'

import { Box, Flex, Text, VStack } from '@devup-ui/react'
import { useRouter } from 'next/navigation'
import { useState } from 'react'

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
      alignItems="center"
      bg="$backgroundSecondary"
      h="100vh"
      justifyContent="center"
    >
      <Box
        bg="$background"
        border="1px solid $border"
        borderRadius="16px"
        boxShadow="0 4px 24px rgba(0, 0, 0, 0.1)"
        maxW="400px"
        p={8}
        w="100%"
      >
        <VStack alignItems="stretch" gap={6}>
          <VStack alignItems="center" gap={2}>
            <Text textAlign="center" typography="heading">
              Yeollin CMS
            </Text>
            <Text color="$textSecondary" textAlign="center" typography="body">
              Sign in to your account
            </Text>
          </VStack>

          <form onSubmit={handleSubmit}>
            <VStack alignItems="stretch" gap={4}>
              {error && (
                <Box
                  bg="$errorLight"
                  border="1px solid $error"
                  borderRadius="8px"
                  p={3}
                >
                  <Text color="$error" typography="body">
                    {error}
                  </Text>
                </Box>
              )}

              <VStack alignItems="stretch" gap={2}>
                <Text as="label" htmlFor="username" typography="label">
                  Username
                </Text>
                <Box
                  _focus={{ borderColor: '$primary' }}
                  _placeholder={{ color: '$textTertiary' }}
                  as="input"
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  fontSize="16px"
                  id="username"
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setUsername(e.target.value)
                  }
                  outline="none"
                  p={3}
                  placeholder="Enter username"
                  required
                  type="text"
                  value={username}
                />
              </VStack>

              <VStack alignItems="stretch" gap={2}>
                <Text as="label" htmlFor="password" typography="label">
                  Password
                </Text>
                <Box
                  _focus={{ borderColor: '$primary' }}
                  _placeholder={{ color: '$textTertiary' }}
                  as="input"
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  fontSize="16px"
                  id="password"
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                    setPassword(e.target.value)
                  }
                  outline="none"
                  p={3}
                  placeholder="Enter password"
                  required
                  type="password"
                  value={password}
                />
              </VStack>

              <Box
                _active={{ transform: 'scale(0.98)' }}
                _hover={{ bg: '$primaryHover' }}
                as="button"
                bg="$primary"
                border="none"
                borderRadius="8px"
                color="white"
                cursor={loading ? 'not-allowed' : 'pointer'}
                disabled={loading}
                fontSize="16px"
                fontWeight="600"
                opacity={loading ? 0.7 : 1}
                p={3}
                transition="all 0.2s ease"
                type="submit"
              >
                {loading ? 'Signing in...' : 'Sign in'}
              </Box>
            </VStack>
          </form>

          <Text color="$textTertiary" textAlign="center" typography="label">
            Use superadmin credentials to sign in
          </Text>
        </VStack>
      </Box>
    </Flex>
  )
}
