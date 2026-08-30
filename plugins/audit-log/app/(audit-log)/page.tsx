'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

interface AuditEvent {
  id: number
  name: string
  payload: unknown
  createdAt: string
}

interface AuditPage {
  events: AuditEvent[]
  total: number
  page: number
  pageSize: number
  retentionDays: number
}

class AuditRequestError extends Error {
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

function parseAuditEvent(value: unknown): AuditEvent | null {
  if (!isRecord(value)) return null
  if (
    typeof value.id !== 'number' ||
    typeof value.name !== 'string' ||
    typeof value.createdAt !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    name: value.name,
    payload: value.payload,
    createdAt: value.createdAt,
  }
}

function parseAuditPage(value: unknown): AuditPage {
  if (!isRecord(value))
    throw new Error('The server returned an invalid audit response.')
  const rawEvents = Array.isArray(value.events) ? value.events : []
  return {
    events: rawEvents.map(parseAuditEvent).filter((event) => event !== null),
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : 1,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 20,
    retentionDays:
      typeof value.retentionDays === 'number' ? value.retentionDays : 90,
  }
}

async function loadAuditPage(
  page: number,
  eventName: string,
): Promise<AuditPage> {
  const params = new URLSearchParams({ page: String(page), pageSize: '20' })
  if (eventName !== '') params.set('eventName', eventName)
  const response = await fetch(`/api/audit-log?${params}`)
  const body = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    const message =
      isRecord(body) && typeof body.error === 'string'
        ? body.error
        : 'Could not load audit history.'
    throw new AuditRequestError(message, response.status)
  }
  return parseAuditPage(body)
}

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return 'Unknown time'
  return date.toLocaleString()
}

function formatPayload(payload: unknown): string {
  return JSON.stringify(payload, null, 2) ?? 'null'
}

export default function AuditLogPage() {
  const [auditPage, setAuditPage] = useState<AuditPage>({
    events: [],
    total: 0,
    page: 1,
    pageSize: 20,
    retentionDays: 90,
  })
  const [page, setPage] = useState(1)
  const [draftFilter, setDraftFilter] = useState('')
  const [eventFilter, setEventFilter] = useState('')
  const [refresh, setRefresh] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [forbidden, setForbidden] = useState(false)

  useEffect(() => {
    let cancelled = false
    void loadAuditPage(page, eventFilter)
      .then((result) => {
        if (cancelled) return
        setAuditPage(result)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setAuditPage((current) => ({ ...current, events: [], total: 0 }))
        setError(
          cause instanceof Error
            ? cause.message
            : 'Could not load audit history.',
        )
        setForbidden(cause instanceof AuditRequestError && cause.status === 403)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [eventFilter, page, refresh])

  function beginLoad() {
    setLoading(true)
    setError('')
  }

  function applyFilter(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    beginLoad()
    setPage(1)
    setEventFilter(draftFilter.trim())
    setRefresh((current) => current + 1)
  }

  function clearFilter() {
    beginLoad()
    setDraftFilter('')
    setEventFilter('')
    setPage(1)
    setRefresh((current) => current + 1)
  }

  function changePage(nextPage: number) {
    beginLoad()
    setPage(nextPage)
  }

  const pageCount = Math.max(1, Math.ceil(auditPage.total / auditPage.pageSize))

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
            <Text typography="heading">Audit log</Text>
            <Text color="$textSecondary" typography="body">
              Review events that explicitly opt into administrator audit
              history.
            </Text>
          </VStack>
          <Box
            _hover={{ bg: '$primaryHover' }}
            as="button"
            bg="$primary"
            border="none"
            borderRadius="8px"
            color="white"
            cursor={loading ? 'not-allowed' : 'pointer'}
            disabled={loading}
            fontWeight="600"
            onClick={() => {
              beginLoad()
              setRefresh((current) => current + 1)
            }}
            px={4}
            py={3}
            type="button"
          >
            {loading ? 'Refreshing...' : 'Refresh'}
          </Box>
        </Flex>

        <Grid gap={4} gridTemplateColumns={['1fr', '1fr 1fr']}>
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={5}
          >
            <VStack alignItems="flex-start" gap={1}>
              <Text color="$textSecondary" typography="label">
                Matching events
              </Text>
              <Text typography="subheading">{auditPage.total}</Text>
            </VStack>
          </Box>
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={5}
          >
            <VStack alignItems="flex-start" gap={1}>
              <Text color="$textSecondary" typography="label">
                Retention policy
              </Text>
              <Text typography="subheading">
                {auditPage.retentionDays} days
              </Text>
            </VStack>
          </Box>
        </Grid>

        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          p={5}
        >
          <Box as="form" onSubmit={applyFilter}>
            <Flex
              alignItems={['stretch', null, 'flex-end']}
              flexDirection={['column', null, 'row']}
              gap={3}
            >
              <VStack alignItems="stretch" flex={1} gap={2}>
                <Text as="label" htmlFor="event-name" typography="label">
                  Exact event name
                </Text>
                <Box
                  _focus={{ borderColor: '$primary' }}
                  as="input"
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  id="event-name"
                  onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                    setDraftFilter(event.target.value)
                  }
                  outline="none"
                  p={3}
                  placeholder="memo.created"
                  type="text"
                  value={draftFilter}
                />
              </VStack>
              <Flex gap={2}>
                <Box
                  _hover={{ bg: '$primaryHover' }}
                  as="button"
                  bg="$primary"
                  border="none"
                  borderRadius="8px"
                  color="white"
                  cursor="pointer"
                  fontWeight="600"
                  px={4}
                  py={3}
                  type="submit"
                >
                  Filter
                </Box>
                <Box
                  _hover={{ borderColor: '$primary', color: '$primary' }}
                  as="button"
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  cursor="pointer"
                  onClick={clearFilter}
                  px={4}
                  py={3}
                  type="button"
                >
                  Clear
                </Box>
              </Flex>
            </Flex>
          </Box>
        </Box>

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
                Audit payloads are restricted to administrators.
              </Text>
            </VStack>
          </Box>
        ) : error !== '' ? (
          <Box
            bg="$errorLight"
            border="1px solid $error"
            borderRadius="12px"
            p={6}
          >
            <Text color="$error" typography="body">
              {error}
            </Text>
          </Box>
        ) : loading && auditPage.events.length === 0 ? (
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={6}
          >
            <Text aria-live="polite" color="$textSecondary" typography="body">
              Loading audit history...
            </Text>
          </Box>
        ) : auditPage.events.length === 0 ? (
          <Box
            bg="$background"
            border="1px dashed $border"
            borderRadius="12px"
            p={8}
          >
            <VStack alignItems="center" gap={2}>
              <Text typography="subheading">No matching audit events</Text>
              <Text color="$textSecondary" textAlign="center" typography="body">
                Perform an auditable action or clear the event-name filter.
              </Text>
            </VStack>
          </Box>
        ) : (
          <VStack alignItems="stretch" gap={3} opacity={loading ? 0.6 : 1}>
            {auditPage.events.map((auditEvent) => (
              <Box
                key={auditEvent.id}
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                p={5}
              >
                <VStack alignItems="stretch" gap={4}>
                  <Flex
                    alignItems={['flex-start', null, 'center']}
                    flexDirection={['column', null, 'row']}
                    gap={2}
                    justifyContent="space-between"
                  >
                    <Box bg="$primaryLight" borderRadius="6px" px={3} py={1}>
                      <Text
                        color="$primary"
                        fontWeight="600"
                        typography="label"
                      >
                        {auditEvent.name}
                      </Text>
                    </Box>
                    <Text color="$textTertiary" typography="label">
                      {formatDate(auditEvent.createdAt)} / #{auditEvent.id}
                    </Text>
                  </Flex>
                  <Box
                    as="pre"
                    bg="$backgroundSecondary"
                    borderRadius="8px"
                    fontFamily="ui-monospace, SFMono-Regular, Consolas, monospace"
                    fontSize="13px"
                    m={0}
                    maxH="320px"
                    overflow="auto"
                    p={4}
                    whiteSpace="pre-wrap"
                  >
                    {formatPayload(auditEvent.payload)}
                  </Box>
                </VStack>
              </Box>
            ))}
          </VStack>
        )}

        {!forbidden && error === '' && auditPage.total > 0 ? (
          <Flex alignItems="center" gap={3} justifyContent="space-between">
            <Box
              _hover={{ borderColor: '$primary', color: '$primary' }}
              as="button"
              bg="$background"
              border="1px solid $border"
              borderRadius="8px"
              color="$text"
              cursor={page <= 1 || loading ? 'not-allowed' : 'pointer'}
              disabled={page <= 1 || loading}
              onClick={() => changePage(page - 1)}
              opacity={page <= 1 || loading ? 0.5 : 1}
              px={4}
              py={2}
              type="button"
            >
              Previous
            </Box>
            <Text color="$textSecondary" typography="label">
              Page {page} of {pageCount}
            </Text>
            <Box
              _hover={{ borderColor: '$primary', color: '$primary' }}
              as="button"
              bg="$background"
              border="1px solid $border"
              borderRadius="8px"
              color="$text"
              cursor={page >= pageCount || loading ? 'not-allowed' : 'pointer'}
              disabled={page >= pageCount || loading}
              onClick={() => changePage(page + 1)}
              opacity={page >= pageCount || loading ? 0.5 : 1}
              px={4}
              py={2}
              type="button"
            >
              Next
            </Box>
          </Flex>
        ) : null}
      </VStack>
    </Flex>
  )
}
