'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useCallback, useEffect, useState } from 'react'

const ROLES = ['admin', 'user'] as const

type Role = (typeof ROLES)[number]

const MIN_PASSWORD_LENGTH = 12

interface Account {
  id: number
  username: string
  role: Role
  createdAt: string
}

interface Feedback {
  kind: 'error' | 'success'
  text: string
}

type ApiResult =
  { ok: true; data: unknown } | { ok: false; status: number; message: string }

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isRole(value: unknown): value is Role {
  return value === 'admin' || value === 'user'
}

function toRole(value: string): Role {
  return isRole(value) ? value : 'user'
}

function roleLabel(role: Role): string {
  return role === 'admin' ? 'Administrator' : 'Standard user'
}

/**
 * The API answers every failure with `{ error, code }`. The `error` string is
 * written for a human — surface it verbatim instead of inventing a message.
 */
function errorMessage(body: unknown, fallback: string): string {
  if (isRecord(body) && typeof body.error === 'string' && body.error !== '') {
    return body.error
  }
  return fallback
}

function parseAccounts(body: unknown): Account[] {
  if (!isRecord(body)) return []
  const rawUsers: unknown[] = Array.isArray(body.users) ? body.users : []
  return rawUsers.filter(isRecord).map((raw) => ({
    id: typeof raw.id === 'number' ? raw.id : 0,
    username: typeof raw.username === 'string' ? raw.username : '',
    role: isRole(raw.role) ? raw.role : 'user',
    createdAt: typeof raw.createdAt === 'string' ? raw.createdAt : '',
  }))
}

function parseTotal(body: unknown, fallback: number): number {
  if (isRecord(body) && typeof body.total === 'number') return body.total
  return fallback
}

function formatDate(value: string): string {
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) return 'Unknown'
  return parsed.toLocaleDateString()
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json()
  } catch {
    return null
  }
}

/**
 * The session travels in a cookie the browser attaches on its own, so these are
 * plain same-origin requests with no client-side token handling.
 */
async function request(path: string, init?: RequestInit): Promise<ApiResult> {
  try {
    const response = await fetch(path, init)
    const body = await readJson(response)

    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        message: errorMessage(
          body,
          `The server rejected the request (HTTP ${response.status}).`,
        ),
      }
    }

    return { ok: true, data: body }
  } catch {
    return {
      ok: false,
      status: 0,
      message: 'Could not reach the server. Check your connection and retry.',
    }
  }
}

function jsonInit(method: string, payload: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  }
}

export default function UsersPage() {
  const [accounts, setAccounts] = useState<Account[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [listError, setListError] = useState('')
  const [forbidden, setForbidden] = useState(false)

  const [newUsername, setNewUsername] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [newRole, setNewRole] = useState<Role>('user')
  const [createError, setCreateError] = useState('')
  const [creating, setCreating] = useState(false)

  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [busyId, setBusyId] = useState<number | null>(null)
  const [resetTarget, setResetTarget] = useState<number | null>(null)
  const [resetPassword, setResetPassword] = useState('')

  const loadAccounts = useCallback(async () => {
    setLoading(true)
    const result = await request('/api/auth/users')

    if (!result.ok) {
      setAccounts([])
      setTotal(0)
      setForbidden(result.status === 403)
      setListError(result.message)
      setLoading(false)
      return
    }

    const parsed = parseAccounts(result.data)
    setAccounts(parsed)
    setTotal(parseTotal(result.data, parsed.length))
    setForbidden(false)
    setListError('')
    setLoading(false)
  }, [])

  useEffect(() => {
    void loadAccounts()
  }, [loadAccounts])

  const handleRefresh = () => {
    setFeedback(null)
    void loadAccounts()
  }

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault()
    setCreateError('')
    setFeedback(null)
    setCreating(true)

    const username = newUsername
    const result = await request(
      '/api/auth/users',
      jsonInit('POST', { username, password: newPassword, role: newRole }),
    )

    if (!result.ok) {
      setCreateError(result.message)
      setCreating(false)
      return
    }

    setNewUsername('')
    setNewPassword('')
    setNewRole('user')
    setFeedback({ kind: 'success', text: `Created the account "${username}".` })
    await loadAccounts()
    setCreating(false)
  }

  const handleRoleChange = async (account: Account, role: Role) => {
    if (role === account.role) return

    setFeedback(null)
    setBusyId(account.id)

    const result = await request(
      `/api/auth/users/${account.id}`,
      jsonInit('PATCH', { role }),
    )

    setFeedback(
      result.ok
        ? {
            kind: 'success',
            text: `${account.username} is now ${roleLabel(role).toLowerCase()}.`,
          }
        : { kind: 'error', text: result.message },
    )

    // Re-read the roster either way: on failure this snaps the control back to
    // the role the server actually kept.
    await loadAccounts()
    setBusyId(null)
  }

  const handleDelete = async (account: Account) => {
    const confirmed = window.confirm(
      `Delete "${account.username}"? This removes their access immediately and cannot be undone.`,
    )
    if (!confirmed) return

    setFeedback(null)
    setBusyId(account.id)

    const result = await request(`/api/auth/users/${account.id}`, {
      method: 'DELETE',
    })

    setFeedback(
      result.ok
        ? {
            kind: 'success',
            text: `Deleted the account "${account.username}".`,
          }
        : { kind: 'error', text: result.message },
    )

    if (result.ok && resetTarget === account.id) {
      setResetTarget(null)
      setResetPassword('')
    }

    await loadAccounts()
    setBusyId(null)
  }

  const handleResetPassword = async (e: React.FormEvent, account: Account) => {
    e.preventDefault()
    setFeedback(null)
    setBusyId(account.id)

    const result = await request(
      `/api/auth/users/${account.id}/password`,
      jsonInit('POST', { newPassword: resetPassword }),
    )

    if (!result.ok) {
      setFeedback({ kind: 'error', text: result.message })
      setBusyId(null)
      return
    }

    setResetTarget(null)
    setResetPassword('')
    setFeedback({
      kind: 'success',
      text: `Set a new password for ${account.username}.`,
    })
    await loadAccounts()
    setBusyId(null)
  }

  const toggleReset = (id: number) => {
    setFeedback(null)
    setResetPassword('')
    setResetTarget(resetTarget === id ? null : id)
  }

  const blocked = forbidden || listError !== ''
  const showInitialLoading = loading && !blocked && accounts.length === 0
  const showEmpty = !loading && !blocked && accounts.length === 0
  const showRoster = !blocked && accounts.length > 0

  return (
    <Box p={6}>
      <VStack alignItems="stretch" gap={6}>
        <Flex
          alignItems={['flex-start', null, 'center']}
          flexDirection={['column', null, 'row']}
          gap={3}
          justifyContent="space-between"
        >
          <VStack alignItems="flex-start" gap={2}>
            <Text typography="heading">Users</Text>
            <Text color="$textSecondary" typography="body">
              Create accounts, change roles, reset passwords, and remove access.
            </Text>
          </VStack>

          <Box
            _hover={{ borderColor: '$primary', color: '$primary' }}
            as="button"
            bg="$background"
            border="1px solid $border"
            borderRadius="8px"
            color="$text"
            cursor={loading ? 'not-allowed' : 'pointer'}
            disabled={loading}
            fontSize="14px"
            fontWeight="500"
            onClick={handleRefresh}
            opacity={loading ? 0.6 : 1}
            px={4}
            py={2}
            transition="all 0.2s ease"
            type="button"
          >
            {loading ? 'Refreshing...' : 'Refresh'}
          </Box>
        </Flex>

        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          p={6}
        >
          <VStack alignItems="stretch" gap={4}>
            <VStack alignItems="flex-start" gap={1}>
              <Text typography="subheading">Create an account</Text>
              <Text color="$textSecondary" typography="body">
                The new account can sign in right away with the password you set
                here.
              </Text>
            </VStack>

            <form onSubmit={handleCreate}>
              <VStack alignItems="stretch" gap={4}>
                {createError && (
                  <Box
                    bg="$errorLight"
                    border="1px solid $error"
                    borderRadius="8px"
                    p={3}
                  >
                    <VStack alignItems="flex-start" gap={1}>
                      <Text color="$error" fontWeight="600" typography="label">
                        Could not create the account
                      </Text>
                      <Text color="$error" typography="body">
                        {createError}
                      </Text>
                    </VStack>
                  </Box>
                )}

                <Grid columns={['1fr', '1fr 1fr', '1fr 1fr 1fr']} gap={4}>
                  <VStack alignItems="stretch" gap={2}>
                    <Text as="label" htmlFor="new-username" typography="label">
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
                      id="new-username"
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        setNewUsername(e.target.value)
                      }
                      outline="none"
                      p={3}
                      placeholder="e.g. editor"
                      required
                      type="text"
                      value={newUsername}
                    />
                  </VStack>

                  <VStack alignItems="stretch" gap={2}>
                    <Text as="label" htmlFor="new-password" typography="label">
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
                      id="new-password"
                      minLength={MIN_PASSWORD_LENGTH}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        setNewPassword(e.target.value)
                      }
                      outline="none"
                      p={3}
                      placeholder="At least 12 characters"
                      required
                      type="password"
                      value={newPassword}
                    />
                    <Text color="$textTertiary" typography="label">
                      Must be at least {MIN_PASSWORD_LENGTH} characters long.
                    </Text>
                  </VStack>

                  <VStack alignItems="stretch" gap={2}>
                    <Text as="label" htmlFor="new-role" typography="label">
                      Role
                    </Text>
                    <Box
                      _focus={{ borderColor: '$primary' }}
                      as="select"
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="8px"
                      color="$text"
                      cursor="pointer"
                      fontSize="16px"
                      id="new-role"
                      onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                        setNewRole(toRole(e.target.value))
                      }
                      outline="none"
                      p={3}
                      value={newRole}
                    >
                      {ROLES.map((role) => (
                        <option key={role} value={role}>
                          {roleLabel(role)}
                        </option>
                      ))}
                    </Box>
                    <Text color="$textTertiary" typography="label">
                      Administrators can manage every account.
                    </Text>
                  </VStack>
                </Grid>

                <Flex justifyContent="flex-end">
                  <Box
                    _active={{ transform: 'scale(0.98)' }}
                    _hover={{ bg: '$primaryHover' }}
                    as="button"
                    bg="$primary"
                    border="none"
                    borderRadius="8px"
                    color="white"
                    cursor={creating ? 'not-allowed' : 'pointer'}
                    disabled={creating}
                    fontSize="16px"
                    fontWeight="600"
                    opacity={creating ? 0.7 : 1}
                    px={6}
                    py={3}
                    transition="all 0.2s ease"
                    type="submit"
                    w={['100%', null, 'auto']}
                  >
                    {creating ? 'Creating...' : 'Create account'}
                  </Box>
                </Flex>
              </VStack>
            </form>
          </VStack>
        </Box>

        <VStack alignItems="stretch" gap={3}>
          <Flex alignItems="center" gap={3} justifyContent="space-between">
            <Text typography="subheading">Accounts</Text>
            {showRoster && (
              <Text color="$textSecondary" typography="label">
                {total} {total === 1 ? 'account' : 'accounts'}
                {loading ? ' - refreshing...' : ''}
              </Text>
            )}
          </Flex>

          {feedback && (
            <Box
              bg={feedback.kind === 'error' ? '$errorLight' : '$primaryLight'}
              border={
                feedback.kind === 'error'
                  ? '1px solid $error'
                  : '1px solid $primary'
              }
              borderRadius="8px"
              p={3}
            >
              <VStack alignItems="flex-start" gap={1}>
                <Text
                  color={feedback.kind === 'error' ? '$error' : '$primary'}
                  fontWeight="600"
                  typography="label"
                >
                  {feedback.kind === 'error' ? 'Request refused' : 'Done'}
                </Text>
                <Text
                  color={feedback.kind === 'error' ? '$error' : '$primary'}
                  typography="body"
                >
                  {feedback.text}
                </Text>
              </VStack>
            </Box>
          )}

          {forbidden && (
            <Box
              bg="$errorLight"
              border="1px solid $error"
              borderRadius="12px"
              p={6}
            >
              <VStack alignItems="flex-start" gap={2}>
                <Text color="$error" typography="subheading">
                  Administrator access required
                </Text>
                <Text color="$error" typography="body">
                  Your account is not allowed to manage users. Ask an
                  administrator to promote you, then reload this page.
                </Text>
                {listError && (
                  <Text color="$error" typography="label">
                    Server said: {listError}
                  </Text>
                )}
              </VStack>
            </Box>
          )}

          {!forbidden && listError !== '' && (
            <Box
              bg="$errorLight"
              border="1px solid $error"
              borderRadius="12px"
              p={6}
            >
              <VStack alignItems="flex-start" gap={3}>
                <Text color="$error" typography="subheading">
                  Could not load the accounts
                </Text>
                <Text color="$error" typography="body">
                  {listError}
                </Text>
                <Box
                  _hover={{ bg: '$primaryHover' }}
                  as="button"
                  bg="$primary"
                  border="none"
                  borderRadius="8px"
                  color="white"
                  cursor={loading ? 'not-allowed' : 'pointer'}
                  disabled={loading}
                  fontSize="14px"
                  fontWeight="600"
                  onClick={handleRefresh}
                  opacity={loading ? 0.7 : 1}
                  px={4}
                  py={2}
                  transition="all 0.2s ease"
                  type="button"
                >
                  {loading ? 'Retrying...' : 'Try again'}
                </Box>
              </VStack>
            </Box>
          )}

          {showInitialLoading && (
            <Box
              bg="$background"
              border="1px solid $border"
              borderRadius="12px"
              p={6}
            >
              <Text color="$textSecondary" typography="body">
                Loading accounts...
              </Text>
            </Box>
          )}

          {showEmpty && (
            <Box
              bg="$backgroundSecondary"
              border="1px solid $border"
              borderRadius="12px"
              p={6}
            >
              <VStack alignItems="flex-start" gap={2}>
                <Text typography="subheading">No accounts yet</Text>
                <Text color="$textSecondary" typography="body">
                  Use the form above to create the first account.
                </Text>
              </VStack>
            </Box>
          )}

          {showRoster && (
            <Flex
              flexDirection="column"
              gap={3}
              opacity={loading ? 0.6 : 1}
              transition="opacity 0.2s ease"
            >
              {accounts.map((account) => (
                <Box
                  key={account.id}
                  _hover={{ borderColor: '$primary' }}
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  p={4}
                  transition="all 0.2s ease"
                >
                  <Flex
                    alignItems={['stretch', null, 'flex-end']}
                    flexDirection={['column', null, 'row']}
                    gap={4}
                    justifyContent="space-between"
                  >
                    <VStack alignItems="flex-start" gap={1}>
                      <Flex alignItems="center" flexWrap="wrap" gap={2}>
                        <Text typography="subheading">{account.username}</Text>
                        <Box
                          bg={
                            account.role === 'admin'
                              ? '$primaryLight'
                              : '$backgroundSecondary'
                          }
                          borderRadius="6px"
                          px={2}
                          py={1}
                        >
                          <Text
                            color={
                              account.role === 'admin'
                                ? '$primary'
                                : '$textSecondary'
                            }
                            typography="label"
                          >
                            {roleLabel(account.role)}
                          </Text>
                        </Box>
                      </Flex>
                      <Text color="$textTertiary" typography="label">
                        Created {formatDate(account.createdAt)}
                      </Text>
                    </VStack>

                    <Flex
                      alignItems={['stretch', null, 'flex-end']}
                      flexDirection={['column', null, 'row']}
                      flexWrap="wrap"
                      gap={2}
                    >
                      <VStack alignItems="stretch" gap={1}>
                        <Text
                          as="label"
                          color="$textTertiary"
                          htmlFor={`role-${account.id}`}
                          typography="label"
                        >
                          Role
                        </Text>
                        <Box
                          _focus={{ borderColor: '$primary' }}
                          as="select"
                          bg="$background"
                          border="1px solid $border"
                          borderRadius="8px"
                          color="$text"
                          cursor={
                            busyId === account.id ? 'not-allowed' : 'pointer'
                          }
                          disabled={busyId === account.id}
                          fontSize="14px"
                          id={`role-${account.id}`}
                          onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
                            void handleRoleChange(
                              account,
                              toRole(e.target.value),
                            )
                          }
                          outline="none"
                          px={3}
                          py={2}
                          value={account.role}
                        >
                          {ROLES.map((role) => (
                            <option key={role} value={role}>
                              {roleLabel(role)}
                            </option>
                          ))}
                        </Box>
                      </VStack>

                      <Box
                        _hover={{ borderColor: '$primary', color: '$primary' }}
                        as="button"
                        bg="$background"
                        border="1px solid $border"
                        borderRadius="8px"
                        color="$text"
                        cursor={
                          busyId === account.id ? 'not-allowed' : 'pointer'
                        }
                        disabled={busyId === account.id}
                        fontSize="14px"
                        fontWeight="500"
                        onClick={() => toggleReset(account.id)}
                        opacity={busyId === account.id ? 0.6 : 1}
                        px={3}
                        py={2}
                        transition="all 0.2s ease"
                        type="button"
                      >
                        {resetTarget === account.id
                          ? 'Cancel reset'
                          : 'Reset password'}
                      </Box>

                      <Box
                        _hover={{ bg: '$errorLight' }}
                        as="button"
                        bg="$background"
                        border="1px solid $error"
                        borderRadius="8px"
                        color="$error"
                        cursor={
                          busyId === account.id ? 'not-allowed' : 'pointer'
                        }
                        disabled={busyId === account.id}
                        fontSize="14px"
                        fontWeight="500"
                        onClick={() => void handleDelete(account)}
                        opacity={busyId === account.id ? 0.6 : 1}
                        px={3}
                        py={2}
                        transition="all 0.2s ease"
                        type="button"
                      >
                        Delete
                      </Box>
                    </Flex>
                  </Flex>

                  {resetTarget === account.id && (
                    <Box borderTop="1px solid $border" mt={4} pt={4}>
                      <form
                        onSubmit={(e) => void handleResetPassword(e, account)}
                      >
                        <VStack alignItems="stretch" gap={2}>
                          <Text
                            as="label"
                            htmlFor={`reset-password-${account.id}`}
                            typography="label"
                          >
                            New password for {account.username}
                          </Text>
                          <Flex
                            flexDirection={['column', null, 'row']}
                            gap={2}
                            w="100%"
                          >
                            <Box
                              _focus={{ borderColor: '$primary' }}
                              _placeholder={{ color: '$textTertiary' }}
                              as="input"
                              bg="$background"
                              border="1px solid $border"
                              borderRadius="8px"
                              color="$text"
                              flex="1"
                              fontSize="16px"
                              id={`reset-password-${account.id}`}
                              minLength={MIN_PASSWORD_LENGTH}
                              onChange={(
                                e: React.ChangeEvent<HTMLInputElement>,
                              ) => setResetPassword(e.target.value)}
                              outline="none"
                              p={3}
                              placeholder="At least 12 characters"
                              required
                              type="password"
                              value={resetPassword}
                            />
                            <Box
                              _active={{ transform: 'scale(0.98)' }}
                              _hover={{ bg: '$primaryHover' }}
                              as="button"
                              bg="$primary"
                              border="none"
                              borderRadius="8px"
                              color="white"
                              cursor={
                                busyId === account.id
                                  ? 'not-allowed'
                                  : 'pointer'
                              }
                              disabled={busyId === account.id}
                              fontSize="14px"
                              fontWeight="600"
                              opacity={busyId === account.id ? 0.7 : 1}
                              px={4}
                              py={3}
                              transition="all 0.2s ease"
                              type="submit"
                            >
                              {busyId === account.id
                                ? 'Saving...'
                                : 'Save password'}
                            </Box>
                          </Flex>
                          <Text color="$textTertiary" typography="label">
                            Must be at least {MIN_PASSWORD_LENGTH} characters
                            long. The account keeps its role and username.
                          </Text>
                        </VStack>
                      </form>
                    </Box>
                  )}
                </Box>
              ))}
            </Flex>
          )}
        </VStack>
      </VStack>
    </Box>
  )
}
