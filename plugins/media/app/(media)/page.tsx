'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useEffect, useRef, useState } from 'react'

interface MediaItem {
  id: string
  reference: string
  originalName: string
  mimeType: string
  sizeBytes: number
  uploadedBy: string
  createdAt: string
  url: string
}

interface MediaPageResponse {
  media: MediaItem[]
  total: number
  page: number
  pageSize: number
}

interface MediaSettings {
  maxUploadMegabytes: number
}

class MediaRequestError extends Error {
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

function parseMediaItem(value: unknown): MediaItem | null {
  if (!isRecord(value)) return null
  if (
    typeof value.id !== 'string' ||
    typeof value.reference !== 'string' ||
    typeof value.originalName !== 'string' ||
    typeof value.mimeType !== 'string' ||
    typeof value.sizeBytes !== 'number' ||
    typeof value.uploadedBy !== 'string' ||
    typeof value.createdAt !== 'string' ||
    typeof value.url !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    reference: value.reference,
    originalName: value.originalName,
    mimeType: value.mimeType,
    sizeBytes: value.sizeBytes,
    uploadedBy: value.uploadedBy,
    createdAt: value.createdAt,
    url: value.url,
  }
}

async function responseError(
  response: Response,
  fallback: string,
): Promise<MediaRequestError> {
  const body = (await response.json().catch(() => null)) as unknown
  const message =
    isRecord(body) && typeof body.error === 'string' ? body.error : fallback
  return new MediaRequestError(message, response.status)
}

async function loadMedia(page: number): Promise<MediaPageResponse> {
  const params = new URLSearchParams({ page: String(page), pageSize: '24' })
  const response = await fetch(`/api/media?${params}`)
  if (!response.ok) {
    throw await responseError(response, 'Could not load the media library.')
  }
  const value = (await response.json()) as unknown
  if (!isRecord(value))
    throw new Error('The server returned invalid media data.')
  const rawMedia = Array.isArray(value.media) ? value.media : []
  return {
    media: rawMedia.map(parseMediaItem).filter((item) => item !== null),
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : page,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 24,
  }
}

async function loadSettings(): Promise<MediaSettings> {
  const response = await fetch('/api/media/settings')
  if (!response.ok) {
    throw await responseError(response, 'Could not load media settings.')
  }
  const value = (await response.json()) as unknown
  if (!isRecord(value) || typeof value.maxUploadMegabytes !== 'number') {
    throw new Error('The server returned invalid media settings.')
  }
  return { maxUploadMegabytes: value.maxUploadMegabytes }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`
}

function formatDate(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString()
}

export default function MediaPage() {
  const fileInput = useRef<HTMLInputElement>(null)
  const [mediaPage, setMediaPage] = useState<MediaPageResponse>({
    media: [],
    total: 0,
    page: 1,
    pageSize: 24,
  })
  const [maxUploadMegabytes, setMaxUploadMegabytes] = useState(5)
  const [page, setPage] = useState(1)
  const [refresh, setRefresh] = useState(0)
  const [loading, setLoading] = useState(true)
  const [uploading, setUploading] = useState(false)
  const [deletingId, setDeletingId] = useState('')
  const [copiedReference, setCopiedReference] = useState('')
  const [error, setError] = useState('')
  const [forbidden, setForbidden] = useState(false)

  useEffect(() => {
    let cancelled = false
    void Promise.all([loadMedia(page), loadSettings()])
      .then(([nextPage, settings]) => {
        if (cancelled) return
        setMediaPage(nextPage)
        setMaxUploadMegabytes(settings.maxUploadMegabytes)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setError(
          cause instanceof Error
            ? cause.message
            : 'Could not load the media library.',
        )
        setForbidden(cause instanceof MediaRequestError && cause.status === 403)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [page, refresh])

  function reload() {
    setLoading(true)
    setError('')
    setRefresh((current) => current + 1)
  }

  async function upload(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const file = fileInput.current?.files?.[0]
    if (!file) {
      setError('Choose an image before uploading.')
      return
    }

    setUploading(true)
    setError('')
    const form = new FormData()
    form.append('file', file)
    try {
      const response = await fetch('/api/media', {
        method: 'POST',
        body: form,
      })
      if (!response.ok) {
        throw await responseError(response, 'Could not upload the image.')
      }
      if (fileInput.current) fileInput.current.value = ''
      setPage(1)
      reload()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not upload the image.',
      )
    } finally {
      setUploading(false)
    }
  }

  async function copyReference(reference: string) {
    try {
      await navigator.clipboard.writeText(reference)
      setCopiedReference(reference)
      setError('')
    } catch {
      setError('Could not copy the reference to the clipboard.')
    }
  }

  async function deleteMedia(item: MediaItem) {
    if (
      !window.confirm(`Delete "${item.originalName}"? This cannot be undone.`)
    ) {
      return
    }
    setDeletingId(item.id)
    setError('')
    try {
      const response = await fetch(`/api/media/${item.id}`, {
        method: 'DELETE',
      })
      if (!response.ok) {
        throw await responseError(response, 'Could not delete the image.')
      }
      if (mediaPage.media.length === 1 && page > 1) {
        setPage(page - 1)
      } else {
        reload()
      }
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not delete the image.',
      )
    } finally {
      setDeletingId('')
    }
  }

  const pageCount = Math.max(1, Math.ceil(mediaPage.total / mediaPage.pageSize))

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
            <Text typography="heading">Media library</Text>
            <Text color="$textSecondary" typography="body">
              Upload images and copy stable references for use in content.
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
            onClick={reload}
            px={4}
            py={3}
            type="button"
          >
            {loading ? 'Refreshing...' : 'Refresh'}
          </Box>
        </Flex>

        <Box
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          p={5}
        >
          <Box as="form" onSubmit={upload}>
            <Flex
              alignItems={['stretch', null, 'flex-end']}
              flexDirection={['column', null, 'row']}
              gap={4}
            >
              <VStack alignItems="stretch" flex={1} gap={2}>
                <Text as="label" htmlFor="media-file" typography="label">
                  Image file
                </Text>
                <Box
                  ref={fileInput}
                  accept="image/jpeg,image/png,image/gif,image/webp"
                  as="input"
                  bg="$backgroundSecondary"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  id="media-file"
                  p={3}
                  required
                  type="file"
                />
                <Text color="$textTertiary" typography="label">
                  JPEG, PNG, GIF, or WebP. Maximum {maxUploadMegabytes} MiB.
                </Text>
              </VStack>
              <Box
                _hover={{ bg: '$primaryHover' }}
                as="button"
                bg="$primary"
                border="none"
                borderRadius="8px"
                color="white"
                cursor={uploading ? 'not-allowed' : 'pointer'}
                disabled={uploading}
                fontWeight="600"
                px={5}
                py={3}
                type="submit"
              >
                {uploading ? 'Uploading...' : 'Upload image'}
              </Box>
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
                Media uploads and library metadata are restricted to
                administrators.
              </Text>
            </VStack>
          </Box>
        ) : error !== '' ? (
          <Box
            aria-live="polite"
            bg="$errorLight"
            border="1px solid $error"
            borderRadius="12px"
            p={4}
          >
            <Text color="$error" typography="body">
              {error}
            </Text>
          </Box>
        ) : null}

        {!forbidden && loading && mediaPage.media.length === 0 ? (
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={8}
          >
            <Text color="$textSecondary" textAlign="center" typography="body">
              Loading media...
            </Text>
          </Box>
        ) : !forbidden && mediaPage.media.length === 0 ? (
          <Box
            bg="$background"
            border="1px dashed $border"
            borderRadius="12px"
            p={8}
          >
            <VStack alignItems="center" gap={2}>
              <Text typography="subheading">No media yet</Text>
              <Text color="$textSecondary" textAlign="center" typography="body">
                Upload the first image to create a reusable media reference.
              </Text>
            </VStack>
          </Box>
        ) : !forbidden ? (
          <Grid
            gap={4}
            gridTemplateColumns={['1fr', 'repeat(2, 1fr)', 'repeat(3, 1fr)']}
            opacity={loading ? 0.6 : 1}
          >
            {mediaPage.media.map((item) => (
              <Box
                key={item.id}
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                overflow="hidden"
              >
                <Box
                  alt={item.originalName}
                  as="img"
                  bg="$backgroundSecondary"
                  h="190px"
                  objectFit="cover"
                  src={item.url}
                  w="100%"
                />
                <VStack alignItems="stretch" gap={3} p={4}>
                  <VStack alignItems="flex-start" gap={1}>
                    <Text fontWeight="600" typography="body">
                      {item.originalName}
                    </Text>
                    <Text color="$textTertiary" typography="label">
                      {formatBytes(item.sizeBytes)} ·{' '}
                      {formatDate(item.createdAt)}
                    </Text>
                  </VStack>
                  <Box
                    as="code"
                    bg="$backgroundSecondary"
                    borderRadius="6px"
                    color="$textSecondary"
                    fontSize="12px"
                    overflow="hidden"
                    p={2}
                    textOverflow="ellipsis"
                    whiteSpace="nowrap"
                  >
                    {item.reference}
                  </Box>
                  <Flex gap={2}>
                    <Box
                      _hover={{ borderColor: '$primary', color: '$primary' }}
                      as="button"
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="6px"
                      color="$text"
                      cursor="pointer"
                      flex={1}
                      onClick={() => void copyReference(item.reference)}
                      px={3}
                      py={2}
                      type="button"
                    >
                      {copiedReference === item.reference
                        ? 'Copied'
                        : 'Copy reference'}
                    </Box>
                    <Box
                      _hover={{ bg: '$errorLight' }}
                      as="button"
                      bg="$background"
                      border="1px solid $error"
                      borderRadius="6px"
                      color="$error"
                      cursor={
                        deletingId === item.id ? 'not-allowed' : 'pointer'
                      }
                      disabled={deletingId === item.id}
                      onClick={() => void deleteMedia(item)}
                      px={3}
                      py={2}
                      type="button"
                    >
                      {deletingId === item.id ? 'Deleting...' : 'Delete'}
                    </Box>
                  </Flex>
                </VStack>
              </Box>
            ))}
          </Grid>
        ) : null}

        {!forbidden && mediaPage.total > 0 ? (
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
              onClick={() => {
                setLoading(true)
                setPage(page - 1)
              }}
              opacity={page <= 1 || loading ? 0.5 : 1}
              px={4}
              py={2}
              type="button"
            >
              Previous
            </Box>
            <Text color="$textSecondary" typography="label">
              {mediaPage.total} items · Page {page} of {pageCount}
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
              onClick={() => {
                setLoading(true)
                setPage(page + 1)
              }}
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
