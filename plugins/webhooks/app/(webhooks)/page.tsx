'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useCallback, useEffect, useState } from 'react'

type DeliveryStatus = 'pending' | 'delivered' | 'dead_letter'

interface Webhook {
  id: string
  name: string
  url: string
  eventNames: string[]
  allowPrivateNetworks: boolean
  timeoutSeconds: number
  enabled: boolean
  hasSecret: boolean
  createdAt: string
  updatedAt: string
}

interface Delivery {
  id: string
  webhookId: string
  eventId: number
  eventName: string
  status: DeliveryStatus
  attempts: number
  maxAttempts: number
  responseStatus: number | null
  lastError: string | null
  createdAt: string
  updatedAt: string
  deliveredAt: string | null
}

interface DeliveryPage {
  deliveries: Delivery[]
  total: number
  page: number
  pageSize: number
}

interface FormState {
  name: string
  url: string
  secret: string
  eventNames: string
  timeoutSeconds: number
  allowPrivateNetworks: boolean
  enabled: boolean
}

interface Feedback {
  kind: 'success' | 'error'
  message: string
}

type ApiResult =
  { ok: true; data: unknown } | { ok: false; status: number; message: string }

const EMPTY_FORM: FormState = {
  name: '',
  url: '',
  secret: '',
  eventNames: '',
  timeoutSeconds: 5,
  allowPrivateNetworks: false,
  enabled: true,
}

const EMPTY_DELIVERY_PAGE: DeliveryPage = {
  deliveries: [],
  total: 0,
  page: 1,
  pageSize: 25,
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isDeliveryStatus(value: unknown): value is DeliveryStatus {
  return value === 'pending' || value === 'delivered' || value === 'dead_letter'
}

function parseWebhook(value: unknown): Webhook | null {
  if (!isRecord(value)) return null
  if (
    typeof value.id !== 'string' ||
    typeof value.name !== 'string' ||
    typeof value.url !== 'string' ||
    !Array.isArray(value.eventNames) ||
    !value.eventNames.every((name) => typeof name === 'string') ||
    typeof value.allowPrivateNetworks !== 'boolean' ||
    typeof value.timeoutSeconds !== 'number' ||
    typeof value.enabled !== 'boolean' ||
    typeof value.hasSecret !== 'boolean' ||
    typeof value.createdAt !== 'string' ||
    typeof value.updatedAt !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    name: value.name,
    url: value.url,
    eventNames: value.eventNames as string[],
    allowPrivateNetworks: value.allowPrivateNetworks,
    timeoutSeconds: value.timeoutSeconds,
    enabled: value.enabled,
    hasSecret: value.hasSecret,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  }
}

function parseDelivery(value: unknown): Delivery | null {
  if (!isRecord(value)) return null
  if (
    typeof value.id !== 'string' ||
    typeof value.webhookId !== 'string' ||
    typeof value.eventId !== 'number' ||
    typeof value.eventName !== 'string' ||
    !isDeliveryStatus(value.status) ||
    typeof value.attempts !== 'number' ||
    typeof value.maxAttempts !== 'number' ||
    (value.responseStatus !== null &&
      typeof value.responseStatus !== 'number') ||
    (value.lastError !== null && typeof value.lastError !== 'string') ||
    typeof value.createdAt !== 'string' ||
    typeof value.updatedAt !== 'string' ||
    (value.deliveredAt !== null && typeof value.deliveredAt !== 'string')
  ) {
    return null
  }
  return {
    id: value.id,
    webhookId: value.webhookId,
    eventId: value.eventId,
    eventName: value.eventName,
    status: value.status,
    attempts: value.attempts,
    maxAttempts: value.maxAttempts,
    responseStatus: value.responseStatus,
    lastError: value.lastError,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
    deliveredAt: value.deliveredAt,
  }
}

function parseWebhooks(value: unknown): Webhook[] {
  if (!isRecord(value) || !Array.isArray(value.webhooks)) {
    throw new Error('The server returned invalid webhook data.')
  }
  return value.webhooks
    .map(parseWebhook)
    .filter((webhook): webhook is Webhook => webhook !== null)
}

function parseDeliveryPage(value: unknown): DeliveryPage {
  if (!isRecord(value) || !Array.isArray(value.deliveries)) {
    throw new Error('The server returned invalid delivery data.')
  }
  return {
    deliveries: value.deliveries
      .map(parseDelivery)
      .filter((delivery): delivery is Delivery => delivery !== null),
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : 1,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 25,
  }
}

function errorMessage(value: unknown, fallback: string): string {
  return isRecord(value) && typeof value.error === 'string'
    ? value.error
    : fallback
}

async function request(path: string, init?: RequestInit): Promise<ApiResult> {
  try {
    const response = await fetch(path, init)
    const data = (await response.json().catch(() => null)) as unknown
    if (!response.ok) {
      return {
        ok: false,
        status: response.status,
        message: errorMessage(
          data,
          `The server rejected the request (HTTP ${response.status}).`,
        ),
      }
    }
    return { ok: true, data }
  } catch {
    return {
      ok: false,
      status: 0,
      message: 'Could not reach the server. Check your connection and retry.',
    }
  }
}

function jsonRequest(method: string, value: unknown): RequestInit {
  return {
    method,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(value),
  }
}

function eventNames(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((name) => name.trim())
    .filter((name) => name !== '')
}

function formatDate(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString()
}

function statusLabel(status: DeliveryStatus): string {
  if (status === 'dead_letter') return 'Dead letter'
  return status === 'delivered' ? 'Delivered' : 'Pending'
}

export default function WebhooksPage() {
  const [webhooks, setWebhooks] = useState<Webhook[]>([])
  const [deliveryPage, setDeliveryPage] =
    useState<DeliveryPage>(EMPTY_DELIVERY_PAGE)
  const [deliveryPageNumber, setDeliveryPageNumber] = useState(1)
  const [statusFilter, setStatusFilter] = useState('')
  const [loading, setLoading] = useState(true)
  const [forbidden, setForbidden] = useState(false)
  const [loadError, setLoadError] = useState('')
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [form, setForm] = useState<FormState>(EMPTY_FORM)
  const [editingId, setEditingId] = useState('')
  const [saving, setSaving] = useState(false)
  const [busyId, setBusyId] = useState('')

  const loadData = useCallback(async () => {
    setLoading(true)
    const params = new URLSearchParams({
      page: String(deliveryPageNumber),
      pageSize: '25',
    })
    if (statusFilter !== '') params.set('status', statusFilter)
    const [webhookResult, deliveryResult] = await Promise.all([
      request('/api/webhooks'),
      request(`/api/webhooks/deliveries?${params}`),
    ])
    if (!webhookResult.ok) {
      setForbidden(webhookResult.status === 403)
      setLoadError(webhookResult.message)
      setLoading(false)
      return
    }
    if (!deliveryResult.ok) {
      setForbidden(deliveryResult.status === 403)
      setLoadError(deliveryResult.message)
      setLoading(false)
      return
    }

    try {
      setWebhooks(parseWebhooks(webhookResult.data))
      setDeliveryPage(parseDeliveryPage(deliveryResult.data))
      setForbidden(false)
      setLoadError('')
    } catch (cause) {
      setLoadError(
        cause instanceof Error ? cause.message : 'Could not load webhooks.',
      )
    } finally {
      setLoading(false)
    }
  }, [deliveryPageNumber, statusFilter])

  useEffect(() => {
    const timer = window.setTimeout(() => void loadData(), 0)
    return () => window.clearTimeout(timer)
  }, [loadData])

  function resetForm() {
    setForm(EMPTY_FORM)
    setEditingId('')
  }

  function beginEdit(webhook: Webhook) {
    setEditingId(webhook.id)
    setForm({
      name: webhook.name,
      url: webhook.url,
      secret: '',
      eventNames: webhook.eventNames.join('\n'),
      timeoutSeconds: webhook.timeoutSeconds,
      allowPrivateNetworks: webhook.allowPrivateNetworks,
      enabled: webhook.enabled,
    })
    setFeedback(null)
    window.scrollTo({ top: 0, behavior: 'smooth' })
  }

  async function saveWebhook(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setFeedback(null)
    const updating = editingId !== ''
    const payload = {
      name: form.name,
      url: form.url,
      secret: updating && form.secret === '' ? null : form.secret,
      eventNames: eventNames(form.eventNames),
      allowPrivateNetworks: form.allowPrivateNetworks,
      timeoutSeconds: form.timeoutSeconds,
      enabled: form.enabled,
    }
    const result = await request(
      updating ? `/api/webhooks/${editingId}` : '/api/webhooks',
      jsonRequest(updating ? 'PUT' : 'POST', payload),
    )
    if (!result.ok) {
      setFeedback({ kind: 'error', message: result.message })
      setSaving(false)
      return
    }
    const name = form.name.trim()
    resetForm()
    setFeedback({
      kind: 'success',
      message: updating
        ? `Updated “${name}”.`
        : `Created “${name}”. Its secret will not be shown again.`,
    })
    await loadData()
    setSaving(false)
  }

  async function deleteWebhook(webhook: Webhook) {
    if (
      !window.confirm(
        `Delete “${webhook.name}” and all of its delivery history? This cannot be undone.`,
      )
    ) {
      return
    }
    setBusyId(webhook.id)
    setFeedback(null)
    const result = await request(`/api/webhooks/${webhook.id}`, {
      method: 'DELETE',
    })
    if (result.ok) {
      if (editingId === webhook.id) resetForm()
      setFeedback({
        kind: 'success',
        message: `Deleted “${webhook.name}”.`,
      })
      await loadData()
    } else {
      setFeedback({ kind: 'error', message: result.message })
    }
    setBusyId('')
  }

  async function retryDelivery(delivery: Delivery) {
    setBusyId(delivery.id)
    setFeedback(null)
    const result = await request(
      `/api/webhooks/deliveries/${delivery.id}/retry`,
      { method: 'POST' },
    )
    setFeedback(
      result.ok
        ? {
            kind: 'success',
            message: `Requeued ${delivery.eventName}. Refresh to follow its result.`,
          }
        : { kind: 'error', message: result.message },
    )
    await loadData()
    setBusyId('')
  }

  const webhookNames = new Map(
    webhooks.map((webhook) => [webhook.id, webhook.name]),
  )
  const enabledCount = webhooks.filter((webhook) => webhook.enabled).length
  const pageCount = Math.max(
    1,
    Math.ceil(deliveryPage.total / deliveryPage.pageSize),
  )

  return (
    <Flex
      alignItems="flex-start"
      bg="$backgroundSecondary"
      justifyContent="center"
      minH="100vh"
      px={4}
      py={8}
    >
      <VStack alignItems="stretch" gap={6} maxW="1120px" w="100%">
        <Flex
          alignItems={['stretch', null, 'center']}
          flexDirection={['column', null, 'row']}
          gap={4}
          justifyContent="space-between"
        >
          <VStack alignItems="flex-start" gap={1}>
            <Text typography="heading">Webhooks</Text>
            <Text color="$textSecondary" typography="body">
              Send committed CMS events to signed HTTP endpoints.
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
            onClick={() => void loadData()}
            px={4}
            py={3}
            type="button"
          >
            {loading ? 'Refreshing...' : 'Refresh'}
          </Box>
        </Flex>

        {forbidden ? (
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
                Endpoint secrets and delivery payloads are restricted to
                administrators.
              </Text>
            </VStack>
          </Box>
        ) : (
          <>
            <Grid gap={4} gridTemplateColumns={['1fr', 'repeat(3, 1fr)']}>
              <Summary label="Endpoints" value={webhooks.length} />
              <Summary label="Enabled" value={enabledCount} />
              <Summary label="Matching deliveries" value={deliveryPage.total} />
            </Grid>

            <Box
              bg="$background"
              border="1px solid $border"
              borderRadius="12px"
              p={[4, 6]}
            >
              <Box as="form" onSubmit={saveWebhook}>
                <VStack alignItems="stretch" gap={5}>
                  <Flex alignItems="center" justifyContent="space-between">
                    <VStack alignItems="flex-start" gap={1}>
                      <Text typography="subheading">
                        {editingId === '' ? 'Add endpoint' : 'Edit endpoint'}
                      </Text>
                      <Text color="$textSecondary" typography="body">
                        Secrets are write-only and must contain at least 32
                        bytes.
                      </Text>
                    </VStack>
                    {editingId !== '' ? (
                      <SecondaryButton onClick={resetForm}>
                        Cancel edit
                      </SecondaryButton>
                    ) : null}
                  </Flex>

                  <Grid gap={4} gridTemplateColumns={['1fr', '1fr 2fr']}>
                    <Field htmlFor="webhook-name" label="Name">
                      <Input
                        id="webhook-name"
                        maxLength={100}
                        onChange={(value) =>
                          setForm((current) => ({ ...current, name: value }))
                        }
                        placeholder="Publishing pipeline"
                        required
                        type="text"
                        value={form.name}
                      />
                    </Field>
                    <Field htmlFor="webhook-url" label="Endpoint URL">
                      <Input
                        id="webhook-url"
                        onChange={(value) =>
                          setForm((current) => ({ ...current, url: value }))
                        }
                        placeholder="https://example.com/hooks/yeollin"
                        required
                        type="url"
                        value={form.url}
                      />
                    </Field>
                  </Grid>

                  <Grid gap={4} gridTemplateColumns={['1fr', '2fr 1fr']}>
                    <Field
                      hint={
                        editingId === ''
                          ? 'Used to calculate HMAC-SHA256.'
                          : 'Leave blank to retain the current secret.'
                      }
                      htmlFor="webhook-secret"
                      label="Signing secret"
                    >
                      <Input
                        autoComplete="new-password"
                        id="webhook-secret"
                        minLength={editingId === '' ? 32 : undefined}
                        onChange={(value) =>
                          setForm((current) => ({ ...current, secret: value }))
                        }
                        placeholder={
                          editingId === ''
                            ? 'At least 32 bytes'
                            : 'Unchanged when blank'
                        }
                        required={editingId === ''}
                        type="password"
                        value={form.secret}
                      />
                    </Field>
                    <Field
                      hint="One exact event name per line. Empty means every event."
                      htmlFor="webhook-events"
                      label="Event filter"
                    >
                      <Box
                        _focus={{ borderColor: '$primary' }}
                        as="textarea"
                        bg="$background"
                        border="1px solid $border"
                        borderRadius="8px"
                        color="$text"
                        id="webhook-events"
                        minH="104px"
                        onChange={(
                          event: React.ChangeEvent<HTMLTextAreaElement>,
                        ) =>
                          setForm((current) => ({
                            ...current,
                            eventNames: event.target.value,
                          }))
                        }
                        outline="none"
                        p={3}
                        placeholder={'content.published\nmedia.uploaded'}
                        resize="vertical"
                        value={form.eventNames}
                      />
                    </Field>
                  </Grid>

                  <Grid gap={4} gridTemplateColumns={['1fr', '1fr 1fr']}>
                    <Field
                      hint="Allowed range: 1–30 seconds."
                      htmlFor="webhook-timeout"
                      label="Per-delivery timeout"
                    >
                      <Input
                        id="webhook-timeout"
                        max={30}
                        min={1}
                        onChange={(value) =>
                          setForm((current) => ({
                            ...current,
                            timeoutSeconds: Number(value),
                          }))
                        }
                        required
                        type="number"
                        value={String(form.timeoutSeconds)}
                      />
                    </Field>
                    <VStack alignItems="stretch" gap={3}>
                      <Checkbox
                        checked={form.enabled}
                        label="Endpoint enabled"
                        onChange={(checked) =>
                          setForm((current) => ({
                            ...current,
                            enabled: checked,
                          }))
                        }
                      />
                      <Box
                        bg="$warningLight"
                        border="1px solid $warning"
                        borderRadius="8px"
                        p={3}
                      >
                        <Checkbox
                          checked={form.allowPrivateNetworks}
                          label="Allow private-network addresses"
                          onChange={(checked) =>
                            setForm((current) => ({
                              ...current,
                              allowPrivateNetworks: checked,
                            }))
                          }
                        />
                        <Text color="$textSecondary" mt={2} typography="label">
                          Opt out of SSRF protection only for a trusted internal
                          receiver.
                        </Text>
                      </Box>
                    </VStack>
                  </Grid>

                  <Flex justifyContent="flex-end">
                    <PrimaryButton disabled={saving} type="submit">
                      {saving
                        ? 'Saving...'
                        : editingId === ''
                          ? 'Add webhook'
                          : 'Save changes'}
                    </PrimaryButton>
                  </Flex>
                </VStack>
              </Box>
            </Box>

            {feedback !== null ? (
              <Box
                aria-live="polite"
                bg={feedback.kind === 'error' ? '$errorLight' : '$successLight'}
                border={
                  feedback.kind === 'error'
                    ? '1px solid $error'
                    : '1px solid $success'
                }
                borderRadius="8px"
                p={4}
              >
                <Text
                  color={feedback.kind === 'error' ? '$error' : '$success'}
                  typography="body"
                >
                  {feedback.message}
                </Text>
              </Box>
            ) : null}

            {loadError !== '' ? (
              <Box
                aria-live="polite"
                bg="$errorLight"
                border="1px solid $error"
                borderRadius="8px"
                p={4}
              >
                <Text color="$error" typography="body">
                  {loadError}
                </Text>
              </Box>
            ) : null}

            <VStack alignItems="stretch" gap={3}>
              <Text typography="subheading">Configured endpoints</Text>
              {loading && webhooks.length === 0 ? (
                <EmptyCard>Loading endpoints...</EmptyCard>
              ) : webhooks.length === 0 ? (
                <EmptyCard>
                  No endpoints yet. Add one above to begin delivering events.
                </EmptyCard>
              ) : (
                <Grid gap={4} gridTemplateColumns={['1fr', 'repeat(2, 1fr)']}>
                  {webhooks.map((webhook) => (
                    <Box
                      key={webhook.id}
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="12px"
                      p={5}
                    >
                      <VStack alignItems="stretch" gap={4}>
                        <Flex
                          alignItems="flex-start"
                          justifyContent="space-between"
                        >
                          <VStack alignItems="flex-start" gap={1}>
                            <Text fontWeight="600" typography="body">
                              {webhook.name}
                            </Text>
                            <Text
                              color={
                                webhook.enabled ? '$success' : '$textTertiary'
                              }
                              typography="label"
                            >
                              {webhook.enabled ? 'Enabled' : 'Disabled'}
                            </Text>
                          </VStack>
                          <Text color="$textTertiary" typography="label">
                            {webhook.timeoutSeconds}s timeout
                          </Text>
                        </Flex>
                        <Box
                          as="code"
                          bg="$backgroundSecondary"
                          borderRadius="6px"
                          color="$textSecondary"
                          fontSize="12px"
                          overflow="hidden"
                          p={3}
                          textOverflow="ellipsis"
                          whiteSpace="nowrap"
                        >
                          {webhook.url}
                        </Box>
                        <Text color="$textSecondary" typography="label">
                          {webhook.eventNames.length === 0
                            ? 'All event names'
                            : webhook.eventNames.join(', ')}
                        </Text>
                        {webhook.allowPrivateNetworks ? (
                          <Text color="$warning" typography="label">
                            Private-network delivery allowed
                          </Text>
                        ) : null}
                        <Flex gap={2}>
                          <SecondaryButton onClick={() => beginEdit(webhook)}>
                            Edit
                          </SecondaryButton>
                          <DangerButton
                            disabled={busyId === webhook.id}
                            onClick={() => void deleteWebhook(webhook)}
                          >
                            {busyId === webhook.id ? 'Deleting...' : 'Delete'}
                          </DangerButton>
                        </Flex>
                      </VStack>
                    </Box>
                  ))}
                </Grid>
              )}
            </VStack>

            <VStack alignItems="stretch" gap={3}>
              <Flex
                alignItems={['stretch', null, 'flex-end']}
                flexDirection={['column', null, 'row']}
                gap={3}
                justifyContent="space-between"
              >
                <VStack alignItems="flex-start" gap={1}>
                  <Text typography="subheading">Delivery history</Text>
                  <Text color="$textSecondary" typography="body">
                    Failed deliveries retry with exponential backoff and stop
                    after five attempts.
                  </Text>
                </VStack>
                <Field htmlFor="delivery-status" label="Status">
                  <Box
                    as="select"
                    bg="$background"
                    border="1px solid $border"
                    borderRadius="8px"
                    color="$text"
                    id="delivery-status"
                    onChange={(event: React.ChangeEvent<HTMLSelectElement>) => {
                      setDeliveryPageNumber(1)
                      setStatusFilter(event.target.value)
                    }}
                    p={3}
                    value={statusFilter}
                  >
                    <option value="">All statuses</option>
                    <option value="pending">Pending</option>
                    <option value="delivered">Delivered</option>
                    <option value="dead_letter">Dead letter</option>
                  </Box>
                </Field>
              </Flex>

              {loading && deliveryPage.deliveries.length === 0 ? (
                <EmptyCard>Loading deliveries...</EmptyCard>
              ) : deliveryPage.deliveries.length === 0 ? (
                <EmptyCard>No deliveries match this view.</EmptyCard>
              ) : (
                <VStack alignItems="stretch" gap={3}>
                  {deliveryPage.deliveries.map((delivery) => (
                    <Box
                      key={delivery.id}
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="10px"
                      p={4}
                    >
                      <Flex
                        alignItems={['stretch', null, 'center']}
                        flexDirection={['column', null, 'row']}
                        gap={4}
                        justifyContent="space-between"
                      >
                        <VStack alignItems="flex-start" flex={1} gap={1}>
                          <Flex alignItems="center" flexWrap="wrap" gap={2}>
                            <Text fontWeight="600" typography="body">
                              {delivery.eventName}
                            </Text>
                            <Box
                              bg={
                                delivery.status === 'delivered'
                                  ? '$successLight'
                                  : delivery.status === 'dead_letter'
                                    ? '$errorLight'
                                    : '$warningLight'
                              }
                              borderRadius="999px"
                              color={
                                delivery.status === 'delivered'
                                  ? '$success'
                                  : delivery.status === 'dead_letter'
                                    ? '$error'
                                    : '$warning'
                              }
                              px={2}
                              py={1}
                            >
                              <Text typography="label">
                                {statusLabel(delivery.status)}
                              </Text>
                            </Box>
                          </Flex>
                          <Text color="$textSecondary" typography="label">
                            {webhookNames.get(delivery.webhookId) ??
                              'Deleted endpoint'}{' '}
                            · attempt {delivery.attempts}/{delivery.maxAttempts}
                            {delivery.responseStatus === null
                              ? ''
                              : ` · HTTP ${delivery.responseStatus}`}
                          </Text>
                          <Text color="$textTertiary" typography="label">
                            {formatDate(delivery.updatedAt)} · event #
                            {delivery.eventId}
                          </Text>
                          {delivery.lastError !== null ? (
                            <Text color="$error" typography="label">
                              {delivery.lastError}
                            </Text>
                          ) : null}
                        </VStack>
                        {delivery.status === 'dead_letter' ? (
                          <SecondaryButton
                            disabled={busyId === delivery.id}
                            onClick={() => void retryDelivery(delivery)}
                          >
                            {busyId === delivery.id ? 'Requeuing...' : 'Retry'}
                          </SecondaryButton>
                        ) : null}
                      </Flex>
                    </Box>
                  ))}
                </VStack>
              )}

              {deliveryPage.total > 0 ? (
                <Flex
                  alignItems="center"
                  gap={3}
                  justifyContent="space-between"
                >
                  <SecondaryButton
                    disabled={deliveryPageNumber <= 1 || loading}
                    onClick={() => setDeliveryPageNumber((page) => page - 1)}
                  >
                    Previous
                  </SecondaryButton>
                  <Text color="$textSecondary" typography="label">
                    {deliveryPage.total} deliveries · Page {deliveryPageNumber}{' '}
                    of {pageCount}
                  </Text>
                  <SecondaryButton
                    disabled={deliveryPageNumber >= pageCount || loading}
                    onClick={() => setDeliveryPageNumber((page) => page + 1)}
                  >
                    Next
                  </SecondaryButton>
                </Flex>
              ) : null}
            </VStack>
          </>
        )}
      </VStack>
    </Flex>
  )
}

function Summary({ label, value }: { label: string; value: number }) {
  return (
    <Box bg="$background" border="1px solid $border" borderRadius="12px" p={5}>
      <VStack alignItems="flex-start" gap={1}>
        <Text color="$textSecondary" typography="label">
          {label}
        </Text>
        <Text typography="subheading">{value}</Text>
      </VStack>
    </Box>
  )
}

function Field({
  children,
  hint,
  htmlFor,
  label,
}: {
  children: React.ReactNode
  hint?: string
  htmlFor: string
  label: string
}) {
  return (
    <VStack alignItems="stretch" gap={2}>
      <Text as="label" htmlFor={htmlFor} typography="label">
        {label}
      </Text>
      {children}
      {hint === undefined ? null : (
        <Text color="$textTertiary" typography="label">
          {hint}
        </Text>
      )}
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

function Checkbox({
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

function EmptyCard({ children }: { children: React.ReactNode }) {
  return (
    <Box bg="$background" border="1px dashed $border" borderRadius="12px" p={7}>
      <Text color="$textSecondary" textAlign="center" typography="body">
        {children}
      </Text>
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

function SecondaryButton({
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

function DangerButton({
  children,
  disabled,
  onClick,
}: {
  children: React.ReactNode
  disabled: boolean
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
      px={3}
      py={2}
      type="button"
    >
      {children}
    </Box>
  )
}
