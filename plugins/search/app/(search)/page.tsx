'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

type ContentStatus = 'draft' | 'published'
type StatusFilter = '' | ContentStatus

interface SearchResult {
  subject: string
  id: string
  collection: string
  title: string
  excerpt: string
  url: string
  status: ContentStatus
  updatedAt: string
  relevance: number
}

interface SearchResponse {
  query: string
  results: SearchResult[]
  total: number
  page: number
  pageSize: number
}

class SearchRequestError extends Error {
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

function isContentStatus(value: unknown): value is ContentStatus {
  return value === 'draft' || value === 'published'
}

function parseResult(value: unknown): SearchResult | null {
  if (!isRecord(value)) return null
  if (
    typeof value.subject !== 'string' ||
    typeof value.id !== 'string' ||
    typeof value.collection !== 'string' ||
    typeof value.title !== 'string' ||
    typeof value.excerpt !== 'string' ||
    typeof value.url !== 'string' ||
    !isContentStatus(value.status) ||
    typeof value.updatedAt !== 'string' ||
    typeof value.relevance !== 'number'
  ) {
    return null
  }
  return {
    subject: value.subject,
    id: value.id,
    collection: value.collection,
    title: value.title,
    excerpt: value.excerpt,
    url: value.url,
    status: value.status,
    updatedAt: value.updatedAt,
    relevance: value.relevance,
  }
}

function parseResponse(value: unknown): SearchResponse {
  if (!isRecord(value) || !Array.isArray(value.results)) {
    throw new Error('The server returned invalid search data.')
  }
  return {
    query: typeof value.query === 'string' ? value.query : '',
    results: value.results
      .map(parseResult)
      .filter((result): result is SearchResult => result !== null),
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : 1,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 20,
  }
}

async function loadSearch(
  query: string,
  page: number,
  status: StatusFilter,
  signal: AbortSignal,
): Promise<SearchResponse> {
  const params = new URLSearchParams({
    q: query,
    page: String(page),
    pageSize: '20',
  })
  if (status !== '') params.set('status', status)
  const response = await fetch(`/api/search?${params}`, { signal })
  const body = (await response.json().catch(() => null)) as unknown
  if (!response.ok) {
    const message =
      isRecord(body) && typeof body.error === 'string'
        ? body.error
        : 'Could not run this search.'
    throw new SearchRequestError(message, response.status)
  }
  return parseResponse(body)
}

function formatDate(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return 'Unknown time'
  return date.toLocaleString()
}

function humanize(value: string): string {
  return value
    .split('-')
    .filter(Boolean)
    .map((word) => word[0]?.toUpperCase() + word.slice(1))
    .join(' ')
}

export default function SearchPage() {
  const [draftQuery, setDraftQuery] = useState('')
  const [activeQuery, setActiveQuery] = useState('')
  const [status, setStatus] = useState<StatusFilter>('')
  const [page, setPage] = useState(1)
  const [refresh, setRefresh] = useState(0)
  const [searchResponse, setSearchResponse] = useState<SearchResponse | null>(
    null,
  )
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [forbidden, setForbidden] = useState(false)

  useEffect(() => {
    if (activeQuery === '') return
    const controller = new AbortController()
    void loadSearch(activeQuery, page, status, controller.signal)
      .then((response) => {
        setSearchResponse(response)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return
        setSearchResponse(null)
        setError(cause instanceof Error ? cause.message : 'Search failed.')
        setForbidden(
          cause instanceof SearchRequestError && cause.status === 403,
        )
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false)
      })
    return () => controller.abort()
  }, [activeQuery, page, refresh, status])

  function beginLoad() {
    setLoading(true)
    setError('')
    setForbidden(false)
  }

  function submitSearch(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const query = draftQuery.trim()
    if (query === '') return
    beginLoad()
    setPage(1)
    setActiveQuery(query)
    setRefresh((current) => current + 1)
  }

  function changeStatus(nextStatus: StatusFilter) {
    setStatus(nextStatus)
    setPage(1)
    if (activeQuery !== '') beginLoad()
  }

  function changePage(nextPage: number) {
    beginLoad()
    setPage(nextPage)
  }

  const pageCount = searchResponse
    ? Math.max(1, Math.ceil(searchResponse.total / searchResponse.pageSize))
    : 1

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
        <VStack alignItems="flex-start" gap={1}>
          <Text typography="heading">Search</Text>
          <Text color="$textSecondary" typography="body">
            Find content by title, slug, author, or any structured field.
          </Text>
        </VStack>

        <Box
          as="form"
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          onSubmit={submitSearch}
          p={[4, null, 5]}
        >
          <Flex
            alignItems={['stretch', null, 'flex-end']}
            flexDirection={['column', null, 'row']}
            gap={3}
          >
            <VStack alignItems="stretch" flex={1} gap={2}>
              <Text as="label" htmlFor="search-query" typography="label">
                Search content
              </Text>
              <Box
                _focus={{ borderColor: '$primary' }}
                as="input"
                autoComplete="off"
                bg="$background"
                border="1px solid $border"
                borderRadius="8px"
                fontSize="16px"
                id="search-query"
                maxLength={200}
                onChange={(event) => setDraftQuery(event.currentTarget.value)}
                p={3}
                placeholder="Try “launch checklist”"
                required
                value={draftQuery}
              />
            </VStack>
            <VStack alignItems="stretch" gap={2} minW={['100%', null, '180px']}>
              <Text as="label" htmlFor="search-status" typography="label">
                Publication status
              </Text>
              <Box
                as="select"
                bg="$background"
                border="1px solid $border"
                borderRadius="8px"
                id="search-status"
                onChange={(event) =>
                  changeStatus(event.currentTarget.value as StatusFilter)
                }
                p={3}
                value={status}
              >
                <option value="">All statuses</option>
                <option value="draft">Draft</option>
                <option value="published">Published</option>
              </Box>
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
              minH="46px"
              px={5}
              type="submit"
            >
              {loading ? 'Searching…' : 'Search'}
            </Box>
          </Flex>
          <Text color="$textTertiary" mt={3} typography="label">
            Partial words are prefix-matched, and multiple terms are combined to
            narrow the result set.
          </Text>
        </Box>

        {activeQuery === '' && !error ? (
          <Grid gap={4} gridTemplateColumns={['1fr', null, '1fr 1fr']}>
            <Box
              bg="$background"
              border="1px solid $border"
              borderRadius="12px"
              p={5}
            >
              <VStack alignItems="flex-start" gap={2}>
                <Text color="$primary" typography="label">
                  INDEXED SOURCE
                </Text>
                <Text typography="subheading">Content</Text>
                <Text color="$textSecondary" typography="body">
                  Titles and every scalar field are searchable, including draft
                  and published entries.
                </Text>
              </VStack>
            </Box>
            <Box
              bg="$background"
              border="1px solid $border"
              borderRadius="12px"
              p={5}
            >
              <VStack alignItems="flex-start" gap={2}>
                <Text color="$primary" typography="label">
                  SEARCH ENGINE
                </Text>
                <Text typography="subheading">SQLite FTS5</Text>
                <Text color="$textSecondary" typography="body">
                  Ranked full-text results stay inside the CMS database and are
                  refreshed with every content write.
                </Text>
              </VStack>
            </Box>
          </Grid>
        ) : null}

        {forbidden ? (
          <Box
            bg="$errorLight"
            border="1px solid $error"
            borderRadius="12px"
            p={5}
            role="alert"
          >
            <VStack alignItems="flex-start" gap={1}>
              <Text color="$error" typography="subheading">
                Administrator access required
              </Text>
              <Text color="$error" typography="body">
                Search includes draft content, so only administrators can use
                this index.
              </Text>
            </VStack>
          </Box>
        ) : error !== '' ? (
          <Box
            bg="$errorLight"
            border="1px solid $error"
            borderRadius="12px"
            p={5}
            role="alert"
          >
            <Flex
              alignItems={['stretch', null, 'center']}
              flexDirection={['column', null, 'row']}
              gap={3}
              justifyContent="space-between"
            >
              <Text color="$error" typography="body">
                {error}
              </Text>
              <Box
                _hover={{ bg: '$backgroundSecondary' }}
                as="button"
                bg="$background"
                border="1px solid $error"
                borderRadius="8px"
                color="$error"
                cursor="pointer"
                onClick={() => {
                  beginLoad()
                  setRefresh((current) => current + 1)
                }}
                px={4}
                py={2}
                type="button"
              >
                Retry
              </Box>
            </Flex>
          </Box>
        ) : null}

        {loading ? (
          <VStack alignItems="stretch" aria-live="polite" gap={3}>
            {[0, 1, 2].map((item) => (
              <Box
                key={item}
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                minH="132px"
                opacity={0.65}
                p={5}
              >
                <Text color="$textTertiary" typography="body">
                  Searching indexed content…
                </Text>
              </Box>
            ))}
          </VStack>
        ) : searchResponse ? (
          <VStack alignItems="stretch" gap={4}>
            <Flex
              alignItems={['flex-start', null, 'center']}
              flexDirection={['column', null, 'row']}
              gap={2}
              justifyContent="space-between"
            >
              <VStack alignItems="flex-start" gap={1}>
                <Text typography="subheading">
                  {searchResponse.total} result
                  {searchResponse.total === 1 ? '' : 's'}
                </Text>
                <Text color="$textSecondary" typography="body">
                  Ranked matches for “{searchResponse.query}”
                </Text>
              </VStack>
              <Text color="$textTertiary" typography="label">
                Page {searchResponse.page} of {pageCount}
              </Text>
            </Flex>

            {searchResponse.results.length === 0 ? (
              <Box
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                p={6}
              >
                <VStack alignItems="flex-start" gap={2}>
                  <Text typography="subheading">No content matched</Text>
                  <Text color="$textSecondary" typography="body">
                    Try fewer terms, a longer word prefix, or a different
                    status.
                  </Text>
                </VStack>
              </Box>
            ) : (
              <VStack alignItems="stretch" gap={3}>
                {searchResponse.results.map((result) => (
                  <Box
                    key={`${result.subject}:${result.id}`}
                    bg="$background"
                    border="1px solid $border"
                    borderRadius="12px"
                    p={5}
                  >
                    <Flex
                      alignItems="flex-start"
                      gap={4}
                      justifyContent="space-between"
                    >
                      <VStack alignItems="flex-start" flex={1} gap={2}>
                        <Flex alignItems="center" flexWrap="wrap" gap={2}>
                          <Text color="$primary" typography="label">
                            CONTENT ·{' '}
                            {humanize(result.collection).toUpperCase()}
                          </Text>
                          <Box
                            bg={
                              result.status === 'published'
                                ? '$successLight'
                                : '$warningLight'
                            }
                            borderRadius="999px"
                            px={2}
                            py={1}
                          >
                            <Text
                              color={
                                result.status === 'published'
                                  ? '$success'
                                  : '$warning'
                              }
                              typography="label"
                            >
                              {humanize(result.status)}
                            </Text>
                          </Box>
                        </Flex>
                        <Text typography="subheading">{result.title}</Text>
                        <Text color="$textSecondary" typography="body">
                          {result.excerpt ||
                            'The matching term appears in the title or metadata.'}
                        </Text>
                        <Text color="$textTertiary" typography="label">
                          Updated {formatDate(result.updatedAt)}
                        </Text>
                      </VStack>
                      <Box
                        _hover={{ bg: '$primaryLight' }}
                        as="a"
                        border="1px solid $primary"
                        borderRadius="8px"
                        color="$primary"
                        flexShrink={0}
                        fontWeight="600"
                        href={result.url}
                        px={3}
                        py={2}
                        textDecoration="none"
                      >
                        Open
                      </Box>
                    </Flex>
                  </Box>
                ))}
              </VStack>
            )}

            {pageCount > 1 ? (
              <Flex alignItems="center" gap={3} justifyContent="flex-end">
                <PageButton
                  disabled={page <= 1}
                  label="Previous"
                  onClick={() => changePage(page - 1)}
                />
                <PageButton
                  disabled={page >= pageCount}
                  label="Next"
                  onClick={() => changePage(page + 1)}
                />
              </Flex>
            ) : null}
          </VStack>
        ) : null}
      </VStack>
    </Flex>
  )
}

function PageButton({
  disabled,
  label,
  onClick,
}: {
  disabled: boolean
  label: string
  onClick: () => void
}) {
  return (
    <Box
      _hover={disabled ? undefined : { bg: '$backgroundSecondary' }}
      as="button"
      bg="$background"
      border="1px solid $border"
      borderRadius="8px"
      color={disabled ? '$textTertiary' : '$text'}
      cursor={disabled ? 'not-allowed' : 'pointer'}
      disabled={disabled}
      onClick={onClick}
      px={4}
      py={2}
      type="button"
    >
      {label}
    </Box>
  )
}
