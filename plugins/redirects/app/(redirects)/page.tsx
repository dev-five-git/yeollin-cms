'use client'

import { Box, Flex, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

interface RedirectRule {
  id: string
  sourcePath: string
  destinationPath: string
  enabled: boolean
  createdBy: string
  createdAt: string
  updatedAt: string
}

interface RuleDraft {
  sourcePath: string
  destinationPath: string
  enabled: boolean
}

const EMPTY_DRAFT: RuleDraft = {
  sourcePath: '',
  destinationPath: '',
  enabled: true,
}

class RequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function parseRule(value: unknown): RedirectRule | null {
  if (
    !isRecord(value) ||
    typeof value.id !== 'string' ||
    typeof value.sourcePath !== 'string' ||
    typeof value.destinationPath !== 'string' ||
    typeof value.enabled !== 'boolean' ||
    typeof value.createdBy !== 'string' ||
    typeof value.createdAt !== 'string' ||
    typeof value.updatedAt !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    sourcePath: value.sourcePath,
    destinationPath: value.destinationPath,
    enabled: value.enabled,
    createdBy: value.createdBy,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  }
}

async function request(path: string, init?: RequestInit): Promise<unknown> {
  const response = await fetch(path, init)
  const body = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    const message =
      isRecord(body) && typeof body.error === 'string'
        ? body.error
        : 'The request could not be completed.'
    throw new RequestError(message, response.status)
  }
  return body
}

async function loadRules(signal: AbortSignal): Promise<RedirectRule[]> {
  const result = await request('/api/redirects', { signal })
  if (!isRecord(result) || !Array.isArray(result.redirects)) {
    throw new Error('The server returned invalid redirect data.')
  }
  return result.redirects
    .map(parseRule)
    .filter((rule): rule is RedirectRule => rule !== null)
}

function draftFor(rule: RedirectRule): RuleDraft {
  return {
    sourcePath: rule.sourcePath,
    destinationPath: rule.destinationPath,
    enabled: rule.enabled,
  }
}

function formatDate(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString()
}

export default function RedirectsPage() {
  const [rules, setRules] = useState<RedirectRule[]>([])
  const [draft, setDraft] = useState<RuleDraft>(EMPTY_DRAFT)
  const [editingId, setEditingId] = useState('')
  const [refreshVersion, setRefreshVersion] = useState(0)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [busyId, setBusyId] = useState('')
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [forbidden, setForbidden] = useState(false)

  useEffect(() => {
    const controller = new AbortController()
    void loadRules(controller.signal)
      .then((nextRules) => {
        if (controller.signal.aborted) return
        setRules(nextRules)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return
        setError(
          cause instanceof Error ? cause.message : 'Could not load redirects.',
        )
        setForbidden(cause instanceof RequestError && cause.status === 403)
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [refreshVersion])

  function refresh() {
    setLoading(true)
    setRefreshVersion((current) => current + 1)
  }

  function resetDraft() {
    setDraft(EMPTY_DRAFT)
    setEditingId('')
    setError('')
    setNotice('')
  }

  function edit(rule: RedirectRule) {
    setDraft(draftFor(rule))
    setEditingId(rule.id)
    setError('')
    setNotice('Editing a live redirect rule.')
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setError('')
    try {
      const path =
        editingId === '' ? '/api/redirects' : `/api/redirects/${editingId}`
      await request(path, {
        method: editingId === '' ? 'POST' : 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(draft),
      })
      setNotice(editingId === '' ? 'Redirect created.' : 'Redirect updated.')
      setDraft(EMPTY_DRAFT)
      setEditingId('')
      refresh()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not save the redirect.',
      )
    } finally {
      setSaving(false)
    }
  }

  async function toggle(rule: RedirectRule) {
    setBusyId(rule.id)
    setError('')
    try {
      await request(`/api/redirects/${rule.id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          sourcePath: rule.sourcePath,
          destinationPath: rule.destinationPath,
          enabled: !rule.enabled,
        }),
      })
      setNotice(rule.enabled ? 'Redirect paused.' : 'Redirect enabled.')
      refresh()
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : 'Could not change the redirect state.',
      )
    } finally {
      setBusyId('')
    }
  }

  async function remove(rule: RedirectRule) {
    if (!window.confirm(`Delete the redirect from "${rule.sourcePath}"?`))
      return
    setBusyId(rule.id)
    setError('')
    try {
      await request(`/api/redirects/${rule.id}`, { method: 'DELETE' })
      if (editingId === rule.id) resetDraft()
      setNotice('Redirect deleted.')
      refresh()
    } catch (cause) {
      setError(
        cause instanceof Error
          ? cause.message
          : 'Could not delete the redirect.',
      )
    } finally {
      setBusyId('')
    }
  }

  return (
    <Flex
      alignItems="flex-start"
      bg="$backgroundSecondary"
      justifyContent="center"
      minH="100vh"
      px={4}
      py={8}
    >
      <VStack alignItems="stretch" gap={6} maxW="1040px" w="100%">
        <Flex
          alignItems={['stretch', null, 'center']}
          flexDirection={['column', null, 'row']}
          gap={4}
          justifyContent="space-between"
        >
          <VStack alignItems="flex-start" gap={1}>
            <Text typography="heading">Redirects</Text>
            <Text color="$textSecondary" typography="body">
              Permanently send retired page URLs to their replacement before
              authentication or static fallback.
            </Text>
          </VStack>
          <Button disabled={loading} onClick={refresh}>
            {loading ? 'Refreshing...' : 'Refresh'}
          </Button>
        </Flex>

        {error !== '' ? <Message tone="error">{error}</Message> : null}
        {notice !== '' ? <Message tone="success">{notice}</Message> : null}
        {forbidden ? (
          <Message tone="error">
            Administrator access is required to manage redirects.
          </Message>
        ) : null}

        {!forbidden ? (
          <Box
            as="form"
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            onSubmit={save}
            p={5}
          >
            <VStack alignItems="stretch" gap={4}>
              <Flex alignItems="center" justifyContent="space-between">
                <VStack alignItems="flex-start" gap={1}>
                  <Text typography="subheading">
                    {editingId === '' ? 'Create redirect' : 'Edit redirect'}
                  </Text>
                  <Text color="$textTertiary" typography="label">
                    Sources are exact root-relative paths. Destinations may be
                    another internal path or an https URL.
                  </Text>
                </VStack>
                {editingId !== '' ? (
                  <Button onClick={resetDraft}>Cancel editing</Button>
                ) : null}
              </Flex>

              <Field htmlFor="redirect-source" label="Source path">
                <Input
                  id="redirect-source"
                  onChange={(value) =>
                    setDraft((current) => ({ ...current, sourcePath: value }))
                  }
                  placeholder="/old-pricing"
                  required
                  value={draft.sourcePath}
                />
              </Field>
              <Field htmlFor="redirect-destination" label="Destination">
                <Input
                  id="redirect-destination"
                  onChange={(value) =>
                    setDraft((current) => ({
                      ...current,
                      destinationPath: value,
                    }))
                  }
                  placeholder="/pricing"
                  required
                  value={draft.destinationPath}
                />
              </Field>
              <CheckBox
                checked={draft.enabled}
                label="Enable this redirect immediately"
                onChange={(enabled) =>
                  setDraft((current) => ({ ...current, enabled }))
                }
              />
              <Flex justifyContent="flex-end">
                <PrimaryButton disabled={saving} type="submit">
                  {saving
                    ? 'Saving...'
                    : editingId === ''
                      ? 'Create redirect'
                      : 'Save changes'}
                </PrimaryButton>
              </Flex>
            </VStack>
          </Box>
        ) : null}

        {!forbidden ? (
          <VStack alignItems="stretch" gap={3}>
            <Text typography="subheading">Configured redirects</Text>
            {loading && rules.length === 0 ? (
              <EmptyState>Loading redirects...</EmptyState>
            ) : rules.length === 0 ? (
              <EmptyState>
                Create a rule for a retired URL before visitors encounter a
                protected or missing page.
              </EmptyState>
            ) : (
              <VStack alignItems="stretch" gap={3}>
                {rules.map((rule) => (
                  <Box
                    key={rule.id}
                    bg="$background"
                    border="1px solid $border"
                    borderRadius="12px"
                    p={5}
                  >
                    <Flex
                      alignItems={['stretch', null, 'center']}
                      flexDirection={['column', null, 'row']}
                      gap={4}
                      justifyContent="space-between"
                    >
                      <VStack alignItems="flex-start" flex={1} gap={2}>
                        <Flex alignItems="center" flexWrap="wrap" gap={2}>
                          <Text typography="subheading">{rule.sourcePath}</Text>
                          <Status enabled={rule.enabled} />
                        </Flex>
                        <Text color="$textSecondary" typography="body">
                          308 → {rule.destinationPath}
                        </Text>
                        <Text color="$textTertiary" typography="label">
                          Updated {formatDate(rule.updatedAt)} by{' '}
                          {rule.createdBy}
                        </Text>
                      </VStack>
                      <Flex flexWrap="wrap" gap={2}>
                        <Button onClick={() => edit(rule)}>Edit</Button>
                        <Button
                          disabled={busyId === rule.id}
                          onClick={() => void toggle(rule)}
                        >
                          {rule.enabled ? 'Pause' : 'Enable'}
                        </Button>
                        <DangerButton
                          disabled={busyId === rule.id}
                          onClick={() => void remove(rule)}
                        >
                          Delete
                        </DangerButton>
                      </Flex>
                    </Flex>
                  </Box>
                ))}
              </VStack>
            )}
          </VStack>
        ) : null}
      </VStack>
    </Flex>
  )
}

function Field({
  children,
  htmlFor,
  label,
}: {
  children: React.ReactNode
  htmlFor: string
  label: string
}) {
  return (
    <VStack alignItems="stretch" gap={2}>
      <Text as="label" htmlFor={htmlFor} typography="label">
        {label}
      </Text>
      {children}
    </VStack>
  )
}

function Input({
  onChange,
  ...props
}: Omit<React.InputHTMLAttributes<HTMLInputElement>, 'onChange'> & {
  onChange: (value: string) => void
}) {
  return (
    <Box
      {...props}
      _focus={{ borderColor: '$primary' }}
      as="input"
      bg="$background"
      border="1px solid $border"
      borderRadius="8px"
      color="$text"
      onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
        onChange(event.target.value)
      }
      outline="none"
      p={3}
    />
  )
}

function CheckBox({
  checked,
  label,
  onChange,
}: {
  checked: boolean
  label: string
  onChange: (checked: boolean) => void
}) {
  return (
    <Flex alignItems="center" as="label" cursor="pointer" gap={2}>
      <Box
        as="input"
        checked={checked}
        onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
          onChange(event.target.checked)
        }
        type="checkbox"
      />
      <Text typography="body">{label}</Text>
    </Flex>
  )
}

function Button({
  children,
  disabled = false,
  onClick,
}: {
  children: React.ReactNode
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Box
      _hover={{ borderColor: '$primary', color: '$primary' }}
      as="button"
      bg="$background"
      border="1px solid $border"
      borderRadius="8px"
      color="$text"
      cursor={disabled ? 'not-allowed' : 'pointer'}
      disabled={disabled}
      onClick={onClick}
      opacity={disabled ? 0.5 : 1}
      px={3}
      py={2}
      type="button"
    >
      {children}
    </Box>
  )
}

function PrimaryButton({
  children,
  disabled = false,
  type = 'button',
}: {
  children: React.ReactNode
  disabled?: boolean
  type?: 'button' | 'submit'
}) {
  return (
    <Box
      _hover={{ bg: '$primaryHover' }}
      as="button"
      bg="$primary"
      border="none"
      borderRadius="8px"
      color="white"
      cursor={disabled ? 'not-allowed' : 'pointer'}
      disabled={disabled}
      fontWeight="600"
      opacity={disabled ? 0.6 : 1}
      px={4}
      py={3}
      type={type}
    >
      {children}
    </Box>
  )
}

function DangerButton({
  children,
  disabled = false,
  onClick,
}: {
  children: React.ReactNode
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <Box
      _hover={{ bg: '$errorLight' }}
      as="button"
      bg="$background"
      border="1px solid $error"
      borderRadius="8px"
      color="$error"
      cursor={disabled ? 'not-allowed' : 'pointer'}
      disabled={disabled}
      onClick={onClick}
      opacity={disabled ? 0.5 : 1}
      px={3}
      py={2}
      type="button"
    >
      {children}
    </Box>
  )
}

function Status({ enabled }: { enabled: boolean }) {
  const color = enabled ? '$success' : '$warning'
  const background = enabled ? '$successLight' : '$warningLight'
  return (
    <Box bg={background} borderRadius="999px" color={color} px={2} py={1}>
      <Text typography="label">{enabled ? 'Enabled' : 'Paused'}</Text>
    </Box>
  )
}

function Message({
  children,
  tone,
}: {
  children: React.ReactNode
  tone: 'error' | 'success'
}) {
  const color = tone === 'error' ? '$error' : '$success'
  const background = tone === 'error' ? '$errorLight' : '$successLight'
  return (
    <Box
      aria-live="polite"
      bg={background}
      border={`1px solid ${color}`}
      borderRadius="10px"
      p={4}
    >
      <Text color={color} typography="body">
        {children}
      </Text>
    </Box>
  )
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <Box bg="$background" border="1px dashed $border" borderRadius="12px" p={7}>
      <Text color="$textSecondary" textAlign="center" typography="body">
        {children}
      </Text>
    </Box>
  )
}
