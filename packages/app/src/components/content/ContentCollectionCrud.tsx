'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useEffect, useId, useState } from 'react'

export interface ContentFieldSchema {
  anyOf?: ContentFieldSchema[]
  description?: string
  enum?: Array<string | number>
  format?: string
  items?: ContentFieldSchema
  properties?: Record<string, ContentFieldSchema>
  required?: string[]
  title?: string
  type?: string | string[]
}

interface ContentEntry {
  id: string
  collection: string
  title: string
  slug: string
  status: 'draft' | 'published'
  author: string
  fields: Record<string, unknown>
  createdAt: string
  updatedAt: string
  publishedAt: string | null
}

interface ContentPage {
  entries: ContentEntry[]
  total: number
  page: number
  pageSize: number
}

interface ContentForm {
  title: string
  slug: string
  fields: Record<string, unknown>
}

interface ContentCollectionCrudProps {
  apiPath: string
  defaultValue: Record<string, unknown>
  label: string
  schema: ContentFieldSchema
}

class ContentRequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function parseEntry(value: unknown): ContentEntry | null {
  if (!isRecord(value) || !isRecord(value.fields)) return null
  if (
    typeof value.id !== 'string' ||
    typeof value.collection !== 'string' ||
    typeof value.title !== 'string' ||
    typeof value.slug !== 'string' ||
    (value.status !== 'draft' && value.status !== 'published') ||
    typeof value.author !== 'string' ||
    typeof value.createdAt !== 'string' ||
    typeof value.updatedAt !== 'string' ||
    (value.publishedAt !== null && typeof value.publishedAt !== 'string')
  ) {
    return null
  }
  return {
    id: value.id,
    collection: value.collection,
    title: value.title,
    slug: value.slug,
    status: value.status,
    author: value.author,
    fields: value.fields,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
    publishedAt: value.publishedAt,
  }
}

function parsePage(value: unknown, requestedPage: number): ContentPage {
  if (!isRecord(value)) {
    throw new Error('The server returned invalid content data.')
  }
  const entries = Array.isArray(value.entries)
    ? value.entries.map(parseEntry).filter((entry) => entry !== null)
    : []
  return {
    entries,
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : requestedPage,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 20,
  }
}

async function requestError(
  response: Response,
  fallback: string,
): Promise<ContentRequestError> {
  const body = (await response.json().catch(() => null)) as unknown
  const message =
    isRecord(body) && typeof body.error === 'string' ? body.error : fallback
  return new ContentRequestError(message, response.status)
}

async function loadPage(
  apiPath: string,
  page: number,
  status: string,
): Promise<ContentPage> {
  const query = new URLSearchParams({ page: String(page), pageSize: '20' })
  if (status !== 'all') query.set('status', status)
  const response = await fetch(`${apiPath}?${query}`)
  if (!response.ok) {
    throw await requestError(response, 'Could not load content.')
  }
  return parsePage(await response.json(), page)
}

async function mutate<T>(
  path: string,
  method: string,
  body?: unknown,
): Promise<T> {
  const response = await fetch(path, {
    body: body === undefined ? undefined : JSON.stringify(body),
    headers:
      body === undefined ? undefined : { 'Content-Type': 'application/json' },
    method,
  })
  if (!response.ok) {
    throw await requestError(response, 'Could not save content.')
  }
  return (await response.json()) as T
}

function cloneFields(value: Record<string, unknown>) {
  return JSON.parse(JSON.stringify(value)) as Record<string, unknown>
}

function emptyForm(defaultValue: Record<string, unknown>): ContentForm {
  return { fields: cloneFields(defaultValue), slug: '', title: '' }
}

function formFromEntry(entry: ContentEntry): ContentForm {
  return {
    fields: cloneFields(entry.fields),
    slug: entry.slug,
    title: entry.title,
  }
}

function fieldLabel(name: string, schema: ContentFieldSchema) {
  if (schema.title) return schema.title
  return name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replaceAll('_', ' ')
    .replace(/^./, (letter) => letter.toUpperCase())
}

function concreteSchema(schema: ContentFieldSchema) {
  return schema.anyOf?.find((candidate) => candidate.type !== 'null') ?? schema
}

function schemaType(schema: ContentFieldSchema) {
  return Array.isArray(schema.type)
    ? schema.type.find((type) => type !== 'null')
    : schema.type
}

function formatDate(value: string | null) {
  if (value === null) return 'Not published'
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString()
}

interface JsonFieldProps {
  id: string
  onChange: (value: unknown) => void
  required: boolean
  value: unknown
}

function JsonField({ id, onChange, required, value }: JsonFieldProps) {
  const [raw, setRaw] = useState(() => JSON.stringify(value, null, 2))
  const [invalid, setInvalid] = useState(false)

  return (
    <VStack alignItems="stretch" gap={1}>
      <Box
        aria-invalid={invalid}
        as="textarea"
        bg="$background"
        border={invalid ? '1px solid $error' : '1px solid $border'}
        borderRadius="8px"
        color="$text"
        fontFamily="ui-monospace, SFMono-Regular, Consolas, monospace"
        id={id}
        minH="128px"
        onChange={(event: React.ChangeEvent<HTMLTextAreaElement>) => {
          const next = event.target.value
          setRaw(next)
          try {
            onChange(JSON.parse(next))
            setInvalid(false)
          } catch {
            setInvalid(true)
          }
        }}
        outline="none"
        p={3}
        required={required}
        value={raw}
      />
      {invalid ? (
        <Text color="$error" typography="label">
          Enter valid JSON before saving.
        </Text>
      ) : null}
    </VStack>
  )
}

interface FieldEditorProps {
  name: string
  onChange: (value: unknown) => void
  required: boolean
  schema: ContentFieldSchema
  value: unknown
}

function FieldEditor({
  name,
  onChange,
  required,
  schema,
  value,
}: FieldEditorProps) {
  const fallbackId = useId()
  const id = `content-${name}-${fallbackId.replaceAll(':', '')}`
  const field = concreteSchema(schema)
  const type = schemaType(field)
  const label = fieldLabel(name, field)

  let control: React.ReactNode
  if (field.enum) {
    control = (
      <Box
        as="select"
        bg="$background"
        border="1px solid $border"
        borderRadius="8px"
        color="$text"
        id={id}
        onChange={(event: React.ChangeEvent<HTMLSelectElement>) =>
          onChange(event.target.value)
        }
        p={3}
        required={required}
        value={String(value ?? '')}
      >
        {!required ? <option value="">None</option> : null}
        {field.enum.map((option) => (
          <option key={String(option)} value={option}>
            {option}
          </option>
        ))}
      </Box>
    )
  } else if (type === 'boolean') {
    control = (
      <Flex alignItems="center" gap={3}>
        <Box
          as="input"
          checked={Boolean(value)}
          id={id}
          onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
            onChange(event.target.checked)
          }
          type="checkbox"
        />
        <Text color="$textSecondary" typography="body">
          Enabled
        </Text>
      </Flex>
    )
  } else if (type === 'number' || type === 'integer') {
    control = (
      <Box
        _focus={{ borderColor: '$primary' }}
        as="input"
        bg="$background"
        border="1px solid $border"
        borderRadius="8px"
        color="$text"
        id={id}
        onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
          onChange(
            event.target.value === '' ? null : event.target.valueAsNumber,
          )
        }
        outline="none"
        p={3}
        required={required}
        step={type === 'integer' ? 1 : 'any'}
        type="number"
        value={typeof value === 'number' ? value : ''}
      />
    )
  } else if (type === 'array' || type === 'object') {
    control = (
      <JsonField
        key={`${id}-${JSON.stringify(value)}`}
        id={id}
        onChange={onChange}
        required={required}
        value={value}
      />
    )
  } else {
    const multiline =
      field.format === 'textarea' || /body|content|description/i.test(name)
    control = multiline ? (
      <Box
        _focus={{ borderColor: '$primary' }}
        as="textarea"
        bg="$background"
        border="1px solid $border"
        borderRadius="8px"
        color="$text"
        id={id}
        minH="144px"
        onChange={(event: React.ChangeEvent<HTMLTextAreaElement>) =>
          onChange(
            event.target.value === '' && !required ? null : event.target.value,
          )
        }
        outline="none"
        p={3}
        required={required}
        value={typeof value === 'string' ? value : ''}
      />
    ) : (
      <Box
        _focus={{ borderColor: '$primary' }}
        as="input"
        bg="$background"
        border="1px solid $border"
        borderRadius="8px"
        color="$text"
        id={id}
        onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
          onChange(
            event.target.value === '' && !required ? null : event.target.value,
          )
        }
        outline="none"
        p={3}
        placeholder={/image|media/i.test(name) ? 'media:0123...' : undefined}
        required={required}
        type="text"
        value={typeof value === 'string' ? value : ''}
      />
    )
  }

  return (
    <VStack alignItems="stretch" gap={2}>
      <Text as="label" htmlFor={id} typography="label">
        {label}
        {required ? ' *' : ''}
      </Text>
      {control}
      {field.description ? (
        <Text color="$textSecondary" typography="label">
          {field.description}
        </Text>
      ) : null}
    </VStack>
  )
}

export function ContentCollectionCrud({
  apiPath,
  defaultValue,
  label,
  schema,
}: ContentCollectionCrudProps) {
  const [contentPage, setContentPage] = useState<ContentPage>({
    entries: [],
    page: 1,
    pageSize: 20,
    total: 0,
  })
  const [page, setPage] = useState(1)
  const [statusFilter, setStatusFilter] = useState('all')
  const [refresh, setRefresh] = useState(0)
  const [selected, setSelected] = useState<ContentEntry | null>(null)
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState<ContentForm>(() => emptyForm(defaultValue))
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [forbidden, setForbidden] = useState(false)

  useEffect(() => {
    let cancelled = false
    void loadPage(apiPath, page, statusFilter)
      .then((result) => {
        if (cancelled) return
        setContentPage(result)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setContentPage((current) => ({ ...current, entries: [], total: 0 }))
        setError(
          cause instanceof Error ? cause.message : 'Could not load content.',
        )
        setForbidden(
          cause instanceof ContentRequestError && cause.status === 403,
        )
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [apiPath, page, refresh, statusFilter])

  function reload() {
    setLoading(true)
    setRefresh((current) => current + 1)
  }

  function startCreate() {
    setCreating(true)
    setSelected(null)
    setForm(emptyForm(defaultValue))
    setError('')
    setNotice('')
  }

  function startEdit(entry: ContentEntry) {
    setCreating(false)
    setSelected(entry)
    setForm(formFromEntry(entry))
    setError('')
    setNotice('')
  }

  function updateField(name: string, value: unknown) {
    setForm((current) => ({
      ...current,
      fields: { ...current.fields, [name]: value },
    }))
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setError('')
    setNotice('')
    try {
      const result = creating
        ? await mutate<unknown>(apiPath, 'POST', form)
        : await mutate<unknown>(`${apiPath}/${selected?.id}`, 'PUT', form)
      const entry = parseEntry(result)
      if (entry === null)
        throw new Error('The server returned invalid content data.')
      setSelected(entry)
      setCreating(false)
      setForm(formFromEntry(entry))
      setNotice(creating ? 'Draft created.' : 'Changes saved.')
      reload()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not save content.',
      )
    } finally {
      setSaving(false)
    }
  }

  async function transition(action: 'publish' | 'unpublish') {
    if (selected === null) return
    setSaving(true)
    setError('')
    setNotice('')
    try {
      const result = await mutate<unknown>(
        `${apiPath}/${selected.id}/${action}`,
        'POST',
      )
      const entry = parseEntry(result)
      if (entry === null)
        throw new Error('The server returned invalid content data.')
      setSelected(entry)
      setForm(formFromEntry(entry))
      setNotice(
        action === 'publish' ? 'Entry published.' : 'Entry returned to draft.',
      )
      reload()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : `Could not ${action} content.`,
      )
    } finally {
      setSaving(false)
    }
  }

  async function remove() {
    if (
      selected === null ||
      !window.confirm(`Delete "${selected.title}"? This cannot be undone.`)
    ) {
      return
    }
    setSaving(true)
    setError('')
    try {
      await mutate<unknown>(`${apiPath}/${selected.id}`, 'DELETE')
      setSelected(null)
      setCreating(false)
      setForm(emptyForm(defaultValue))
      setNotice('Entry deleted.')
      if (contentPage.entries.length === 1 && page > 1) setPage(page - 1)
      else reload()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not delete content.',
      )
    } finally {
      setSaving(false)
    }
  }

  const pageCount = Math.max(
    1,
    Math.ceil(contentPage.total / contentPage.pageSize),
  )
  const fields = schema.properties ?? {}
  const editorOpen = creating || selected !== null

  return (
    <Box p={[4, null, 8]}>
      <VStack alignItems="stretch" gap={6} maxW="1280px">
        <Flex
          alignItems={['stretch', null, 'center']}
          flexDirection={['column', null, 'row']}
          gap={4}
          justifyContent="space-between"
        >
          <VStack alignItems="flex-start" gap={1}>
            <Text typography="heading">{label}</Text>
            <Text color="$textSecondary" typography="body">
              Manage typed drafts and make only reviewed entries public.
            </Text>
          </VStack>
          <Flex gap={2}>
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
            <Box
              _hover={{ bg: '$primaryHover' }}
              as="button"
              bg="$primary"
              border="none"
              borderRadius="8px"
              color="white"
              cursor="pointer"
              fontWeight="600"
              onClick={startCreate}
              px={4}
              py={3}
              type="button"
            >
              New draft
            </Box>
          </Flex>
        </Flex>

        {forbidden ? (
          <Box
            bg="$errorLight"
            border="1px solid $error"
            borderRadius="12px"
            p={6}
          >
            <Text color="$error" typography="subheading">
              Administrator access required
            </Text>
          </Box>
        ) : null}
        {!forbidden && error !== '' ? (
          <Box
            aria-live="assertive"
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
        {notice !== '' ? (
          <Box
            aria-live="polite"
            bg="$successLight"
            border="1px solid $success"
            borderRadius="12px"
            p={4}
          >
            <Text color="$success" typography="body">
              {notice}
            </Text>
          </Box>
        ) : null}

        {!forbidden ? (
          <Grid
            gap={5}
            gridTemplateColumns={
              editorOpen
                ? ['1fr', null, 'minmax(300px, 0.8fr) minmax(420px, 1.2fr)']
                : '1fr'
            }
          >
            <VStack alignItems="stretch" gap={4}>
              <Flex alignItems="center" gap={3} justifyContent="space-between">
                <Text typography="subheading">
                  {contentPage.total}{' '}
                  {contentPage.total === 1 ? 'entry' : 'entries'}
                </Text>
                <Box
                  as="select"
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="8px"
                  color="$text"
                  onChange={(event: React.ChangeEvent<HTMLSelectElement>) => {
                    setLoading(true)
                    setPage(1)
                    setStatusFilter(event.target.value)
                  }}
                  px={3}
                  py={2}
                  value={statusFilter}
                >
                  <option value="all">All statuses</option>
                  <option value="draft">Drafts</option>
                  <option value="published">Published</option>
                </Box>
              </Flex>

              {loading && contentPage.entries.length === 0 ? (
                <Box
                  bg="$background"
                  border="1px solid $border"
                  borderRadius="12px"
                  p={8}
                >
                  <Text
                    color="$textSecondary"
                    textAlign="center"
                    typography="body"
                  >
                    Loading content...
                  </Text>
                </Box>
              ) : contentPage.entries.length === 0 ? (
                <Box
                  bg="$background"
                  border="1px dashed $border"
                  borderRadius="12px"
                  p={8}
                >
                  <VStack alignItems="center" gap={2}>
                    <Text typography="subheading">No entries yet</Text>
                    <Text
                      color="$textSecondary"
                      textAlign="center"
                      typography="body"
                    >
                      Create the first draft in this collection.
                    </Text>
                  </VStack>
                </Box>
              ) : (
                <VStack
                  alignItems="stretch"
                  gap={2}
                  opacity={loading ? 0.6 : 1}
                >
                  {contentPage.entries.map((entry) => (
                    <Box
                      key={entry.id}
                      _hover={{ borderColor: '$primary' }}
                      as="button"
                      bg={
                        selected?.id === entry.id
                          ? '$primaryLight'
                          : '$background'
                      }
                      border={
                        selected?.id === entry.id
                          ? '1px solid $primary'
                          : '1px solid $border'
                      }
                      borderRadius="10px"
                      color="$text"
                      cursor="pointer"
                      onClick={() => startEdit(entry)}
                      p={4}
                      textAlign="left"
                      type="button"
                      w="100%"
                    >
                      <VStack alignItems="stretch" gap={2}>
                        <Flex
                          alignItems="flex-start"
                          gap={3}
                          justifyContent="space-between"
                        >
                          <Text fontWeight="600" typography="body">
                            {entry.title}
                          </Text>
                          <Box
                            bg={
                              entry.status === 'published'
                                ? '$successLight'
                                : '$backgroundSecondary'
                            }
                            borderRadius="999px"
                            px={2}
                            py={1}
                          >
                            <Text
                              color={
                                entry.status === 'published'
                                  ? '$success'
                                  : '$textSecondary'
                              }
                              typography="label"
                            >
                              {entry.status}
                            </Text>
                          </Box>
                        </Flex>
                        <Text color="$textTertiary" typography="label">
                          /{entry.slug} / Updated {formatDate(entry.updatedAt)}
                        </Text>
                      </VStack>
                    </Box>
                  ))}
                </VStack>
              )}

              {contentPage.total > 0 ? (
                <Flex
                  alignItems="center"
                  gap={3}
                  justifyContent="space-between"
                >
                  <Box
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
                    Page {page} of {pageCount}
                  </Text>
                  <Box
                    as="button"
                    bg="$background"
                    border="1px solid $border"
                    borderRadius="8px"
                    color="$text"
                    cursor={
                      page >= pageCount || loading ? 'not-allowed' : 'pointer'
                    }
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

            {editorOpen ? (
              <Box
                bg="$background"
                border="1px solid $border"
                borderRadius="12px"
                p={[4, null, 6]}
              >
                <Box as="form" onSubmit={save}>
                  <VStack alignItems="stretch" gap={5}>
                    <Flex
                      alignItems="center"
                      gap={3}
                      justifyContent="space-between"
                    >
                      <VStack alignItems="flex-start" gap={1}>
                        <Text typography="subheading">
                          {creating ? 'New draft' : 'Edit entry'}
                        </Text>
                        {!creating && selected ? (
                          <Text color="$textTertiary" typography="label">
                            Author {selected.author} / Published{' '}
                            {formatDate(selected.publishedAt)}
                          </Text>
                        ) : null}
                      </VStack>
                      <Box
                        as="button"
                        bg="transparent"
                        border="none"
                        color="$textSecondary"
                        cursor="pointer"
                        onClick={() => {
                          setCreating(false)
                          setSelected(null)
                        }}
                        type="button"
                      >
                        Close
                      </Box>
                    </Flex>

                    <VStack alignItems="stretch" gap={2}>
                      <Text
                        as="label"
                        htmlFor="content-title"
                        typography="label"
                      >
                        Title *
                      </Text>
                      <Box
                        _focus={{ borderColor: '$primary' }}
                        as="input"
                        bg="$background"
                        border="1px solid $border"
                        borderRadius="8px"
                        color="$text"
                        id="content-title"
                        onChange={(
                          event: React.ChangeEvent<HTMLInputElement>,
                        ) =>
                          setForm((current) => ({
                            ...current,
                            title: event.target.value,
                          }))
                        }
                        outline="none"
                        p={3}
                        required
                        value={form.title}
                      />
                    </VStack>
                    <VStack alignItems="stretch" gap={2}>
                      <Text
                        as="label"
                        htmlFor="content-slug"
                        typography="label"
                      >
                        Slug *
                      </Text>
                      <Box
                        _focus={{ borderColor: '$primary' }}
                        as="input"
                        bg="$background"
                        border="1px solid $border"
                        borderRadius="8px"
                        color="$text"
                        id="content-slug"
                        onChange={(
                          event: React.ChangeEvent<HTMLInputElement>,
                        ) =>
                          setForm((current) => ({
                            ...current,
                            slug: event.target.value,
                          }))
                        }
                        outline="none"
                        p={3}
                        placeholder="about-us"
                        required
                        value={form.slug}
                      />
                    </VStack>

                    {Object.entries(fields).map(([name, field]) => (
                      <FieldEditor
                        key={name}
                        name={name}
                        onChange={(value) => updateField(name, value)}
                        required={schema.required?.includes(name) ?? false}
                        schema={field}
                        value={form.fields[name]}
                      />
                    ))}

                    <Flex flexWrap="wrap" gap={2}>
                      <Box
                        _hover={{ bg: '$primaryHover' }}
                        as="button"
                        bg="$primary"
                        border="none"
                        borderRadius="8px"
                        color="white"
                        cursor={saving ? 'not-allowed' : 'pointer'}
                        disabled={saving}
                        fontWeight="600"
                        px={4}
                        py={3}
                        type="submit"
                      >
                        {saving
                          ? 'Saving...'
                          : creating
                            ? 'Create draft'
                            : 'Save changes'}
                      </Box>
                      {!creating && selected?.status === 'draft' ? (
                        <Box
                          as="button"
                          bg="$success"
                          border="none"
                          borderRadius="8px"
                          color="white"
                          cursor={saving ? 'not-allowed' : 'pointer'}
                          disabled={saving}
                          onClick={() => void transition('publish')}
                          px={4}
                          py={3}
                          type="button"
                        >
                          Publish
                        </Box>
                      ) : null}
                      {!creating && selected?.status === 'published' ? (
                        <Box
                          as="button"
                          bg="$warning"
                          border="none"
                          borderRadius="8px"
                          color="white"
                          cursor={saving ? 'not-allowed' : 'pointer'}
                          disabled={saving}
                          onClick={() => void transition('unpublish')}
                          px={4}
                          py={3}
                          type="button"
                        >
                          Return to draft
                        </Box>
                      ) : null}
                      {!creating ? (
                        <Box
                          as="button"
                          bg="$background"
                          border="1px solid $error"
                          borderRadius="8px"
                          color="$error"
                          cursor={saving ? 'not-allowed' : 'pointer'}
                          disabled={saving}
                          onClick={() => void remove()}
                          px={4}
                          py={3}
                          type="button"
                        >
                          Delete
                        </Box>
                      ) : null}
                    </Flex>
                  </VStack>
                </Box>
              </Box>
            ) : null}
          </Grid>
        ) : null}
      </VStack>
    </Box>
  )
}
