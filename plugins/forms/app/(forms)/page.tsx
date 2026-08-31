'use client'

import { Box, Flex, Grid, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

type FieldKind = 'text' | 'email' | 'textarea' | 'checkbox' | 'select'

interface FormField {
  id: string
  label: string
  kind: FieldKind
  required: boolean
  options: string[]
  placeholder: string | null
}

interface FormDefinition {
  id: string
  name: string
  description: string
  fields: FormField[]
  successMessage: string
  enabled: boolean
  maxSubmissionsPerHour: number
  createdBy: string
  createdAt: string
  updatedAt: string
}

interface Submission {
  id: string
  formId: string
  formName: string
  fields: FormField[]
  values: Record<string, unknown>
  createdAt: string
}

interface SubmissionPage {
  submissions: Submission[]
  total: number
  page: number
  pageSize: number
}

interface DraftField {
  id: string
  label: string
  kind: FieldKind
  required: boolean
  placeholder: string
  options: string
}

interface FormDraft {
  name: string
  description: string
  fields: DraftField[]
  successMessage: string
  enabled: boolean
  maxSubmissionsPerHour: number
}

class RequestError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message)
  }
}

const EMPTY_DRAFT: FormDraft = {
  name: '',
  description: '',
  fields: [newDraftField('name', 'Your name')],
  successMessage: 'Thanks - we received your response.',
  enabled: true,
  maxSubmissionsPerHour: 100,
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function parseField(value: unknown): FormField | null {
  if (!isRecord(value)) return null
  if (
    typeof value.id !== 'string' ||
    typeof value.label !== 'string' ||
    !isFieldKind(value.kind) ||
    typeof value.required !== 'boolean' ||
    !Array.isArray(value.options) ||
    !value.options.every((option) => typeof option === 'string') ||
    (value.placeholder !== null && typeof value.placeholder !== 'string')
  ) {
    return null
  }
  return {
    id: value.id,
    label: value.label,
    kind: value.kind,
    required: value.required,
    options: value.options,
    placeholder: value.placeholder,
  }
}

function isFieldKind(value: unknown): value is FieldKind {
  return (
    value === 'text' ||
    value === 'email' ||
    value === 'textarea' ||
    value === 'checkbox' ||
    value === 'select'
  )
}

function parseForm(value: unknown): FormDefinition | null {
  if (!isRecord(value) || !Array.isArray(value.fields)) return null
  const fields = value.fields.map(parseField)
  if (
    fields.some((field) => field === null) ||
    typeof value.id !== 'string' ||
    typeof value.name !== 'string' ||
    typeof value.description !== 'string' ||
    typeof value.successMessage !== 'string' ||
    typeof value.enabled !== 'boolean' ||
    typeof value.maxSubmissionsPerHour !== 'number' ||
    typeof value.createdBy !== 'string' ||
    typeof value.createdAt !== 'string' ||
    typeof value.updatedAt !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    name: value.name,
    description: value.description,
    fields: fields.filter((field): field is FormField => field !== null),
    successMessage: value.successMessage,
    enabled: value.enabled,
    maxSubmissionsPerHour: value.maxSubmissionsPerHour,
    createdBy: value.createdBy,
    createdAt: value.createdAt,
    updatedAt: value.updatedAt,
  }
}

function parseSubmission(value: unknown): Submission | null {
  if (
    !isRecord(value) ||
    !Array.isArray(value.fields) ||
    !isRecord(value.values)
  ) {
    return null
  }
  const fields = value.fields.map(parseField)
  if (
    fields.some((field) => field === null) ||
    typeof value.id !== 'string' ||
    typeof value.formId !== 'string' ||
    typeof value.formName !== 'string' ||
    typeof value.createdAt !== 'string'
  ) {
    return null
  }
  return {
    id: value.id,
    formId: value.formId,
    formName: value.formName,
    fields: fields.filter((field): field is FormField => field !== null),
    values: value.values,
    createdAt: value.createdAt,
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

async function loadForms(): Promise<FormDefinition[]> {
  const value = await request('/api/forms')
  if (!isRecord(value) || !Array.isArray(value.forms)) {
    throw new Error('The server returned invalid form data.')
  }
  return value.forms
    .map(parseForm)
    .filter((form): form is FormDefinition => form !== null)
}

async function loadSubmissions(formId: string): Promise<SubmissionPage> {
  const value = await request(`/api/forms/${formId}/submissions?pageSize=50`)
  if (!isRecord(value) || !Array.isArray(value.submissions)) {
    throw new Error('The server returned invalid submission data.')
  }
  return {
    submissions: value.submissions
      .map(parseSubmission)
      .filter((submission): submission is Submission => submission !== null),
    total: typeof value.total === 'number' ? value.total : 0,
    page: typeof value.page === 'number' ? value.page : 1,
    pageSize: typeof value.pageSize === 'number' ? value.pageSize : 50,
  }
}

function newDraftField(id = '', label = ''): DraftField {
  return {
    id,
    label,
    kind: 'text',
    required: false,
    placeholder: '',
    options: '',
  }
}

function draftFromForm(form: FormDefinition): FormDraft {
  return {
    name: form.name,
    description: form.description,
    fields: form.fields.map((field) => ({
      id: field.id,
      label: field.label,
      kind: field.kind,
      required: field.required,
      placeholder: field.placeholder ?? '',
      options: field.options.join('\n'),
    })),
    successMessage: form.successMessage,
    enabled: form.enabled,
    maxSubmissionsPerHour: form.maxSubmissionsPerHour,
  }
}

function wireFields(fields: DraftField[]): FormField[] {
  return fields.map((field) => ({
    id: field.id,
    label: field.label,
    kind: field.kind,
    required: field.required,
    placeholder: field.placeholder.trim() === '' ? null : field.placeholder,
    options:
      field.kind === 'select'
        ? field.options
            .split('\n')
            .map((option) => option.trim())
            .filter((option) => option !== '')
        : [],
  }))
}

function formatDate(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? 'Unknown time' : date.toLocaleString()
}

function displayValue(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'boolean') return value ? 'Yes' : 'No'
  return JSON.stringify(value)
}

export default function FormsPage() {
  const [forms, setForms] = useState<FormDefinition[]>([])
  const [draft, setDraft] = useState<FormDraft>(EMPTY_DRAFT)
  const [editingId, setEditingId] = useState('')
  const [submissions, setSubmissions] = useState<
    Record<string, SubmissionPage>
  >({})
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [busyId, setBusyId] = useState('')
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [forbidden, setForbidden] = useState(false)

  function refresh() {
    setLoading(true)
    void loadForms()
      .then((nextForms) => {
        setForms(nextForms)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        setError(
          cause instanceof Error ? cause.message : 'Could not load forms.',
        )
        setForbidden(cause instanceof RequestError && cause.status === 403)
      })
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    let cancelled = false
    void loadForms()
      .then((nextForms) => {
        if (cancelled) return
        setForms(nextForms)
        setError('')
        setForbidden(false)
      })
      .catch((cause: unknown) => {
        if (cancelled) return
        setError(
          cause instanceof Error ? cause.message : 'Could not load forms.',
        )
        setForbidden(cause instanceof RequestError && cause.status === 403)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  function updateField(index: number, patch: Partial<DraftField>) {
    setDraft((current) => ({
      ...current,
      fields: current.fields.map((field, fieldIndex) =>
        fieldIndex === index ? { ...field, ...patch } : field,
      ),
    }))
  }

  function beginEdit(form: FormDefinition) {
    setEditingId(form.id)
    setDraft(draftFromForm(form))
    setError('')
    setNotice(
      'Editing the live field definition. Existing submissions keep their original snapshot.',
    )
  }

  function resetDraft() {
    setEditingId('')
    setDraft({ ...EMPTY_DRAFT, fields: [newDraftField('name', 'Your name')] })
    setError('')
    setNotice('')
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setError('')
    const payload = {
      ...draft,
      fields: wireFields(draft.fields),
    }
    try {
      const path = editingId === '' ? '/api/forms' : `/api/forms/${editingId}`
      const method = editingId === '' ? 'POST' : 'PUT'
      await request(path, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })
      setNotice(editingId === '' ? 'Form created.' : 'Form updated.')
      resetDraft()
      refresh()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not save the form.',
      )
    } finally {
      setSaving(false)
    }
  }

  async function remove(form: FormDefinition) {
    if (!window.confirm(`Delete "${form.name}" and all of its submissions?`))
      return
    setBusyId(form.id)
    setError('')
    try {
      await request(`/api/forms/${form.id}`, { method: 'DELETE' })
      if (editingId === form.id) resetDraft()
      setNotice('Form and its submissions were deleted.')
      refresh()
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not delete the form.',
      )
    } finally {
      setBusyId('')
    }
  }

  async function toggleSubmissions(form: FormDefinition) {
    if (submissions[form.id] !== undefined) {
      setSubmissions((current) => {
        const next = { ...current }
        delete next[form.id]
        return next
      })
      return
    }
    setBusyId(form.id)
    setError('')
    try {
      const page = await loadSubmissions(form.id)
      setSubmissions((current) => ({ ...current, [form.id]: page }))
    } catch (cause) {
      setError(
        cause instanceof Error ? cause.message : 'Could not load submissions.',
      )
    } finally {
      setBusyId('')
    }
  }

  async function copyPublicId(id: string) {
    try {
      await navigator.clipboard.writeText(id)
      setNotice('Public form id copied to the clipboard.')
      setError('')
    } catch {
      setError('Could not copy the public form id.')
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
      <VStack alignItems="stretch" gap={6} maxW="1180px" w="100%">
        <Flex
          alignItems={['stretch', null, 'center']}
          flexDirection={['column', null, 'row']}
          gap={4}
          justifyContent="space-between"
        >
          <VStack alignItems="flex-start" gap={1}>
            <Text typography="heading">Forms</Text>
            <Text color="$textSecondary" typography="body">
              Build validated public forms and review submitted responses.
            </Text>
          </VStack>
          <SecondaryButton disabled={loading} onClick={refresh}>
            {loading ? 'Refreshing...' : 'Refresh'}
          </SecondaryButton>
        </Flex>

        {forbidden ? (
          <Message tone="error">
            Administrator access is required to manage forms and submissions.
          </Message>
        ) : null}
        {error !== '' ? <Message tone="error">{error}</Message> : null}
        {notice !== '' ? <Message tone="success">{notice}</Message> : null}

        {!forbidden ? (
          <Box
            bg="$background"
            border="1px solid $border"
            borderRadius="12px"
            p={5}
          >
            <Box as="form" onSubmit={save}>
              <VStack alignItems="stretch" gap={5}>
                <Flex
                  alignItems={['stretch', null, 'center']}
                  flexDirection={['column', null, 'row']}
                  gap={3}
                  justifyContent="space-between"
                >
                  <VStack alignItems="flex-start" gap={1}>
                    <Text typography="subheading">
                      {editingId === '' ? 'Create a form' : 'Edit form'}
                    </Text>
                    <Text color="$textSecondary" typography="label">
                      Public clients load enabled forms from
                      /api/forms/public?id=… and submit to /api/forms/submit.
                    </Text>
                  </VStack>
                  {editingId !== '' ? (
                    <SecondaryButton onClick={resetDraft}>
                      Cancel editing
                    </SecondaryButton>
                  ) : null}
                </Flex>

                <Grid gap={4} gridTemplateColumns={['1fr', 'repeat(2, 1fr)']}>
                  <Field htmlFor="form-name" label="Form name">
                    <Input
                      id="form-name"
                      onChange={(value) =>
                        setDraft((current) => ({ ...current, name: value }))
                      }
                      placeholder="Contact us"
                      required
                      value={draft.name}
                    />
                  </Field>
                  <Field htmlFor="form-limit" label="Hourly submission limit">
                    <Input
                      id="form-limit"
                      min="1"
                      onChange={(value) =>
                        setDraft((current) => ({
                          ...current,
                          maxSubmissionsPerHour: Number(value),
                        }))
                      }
                      required
                      type="number"
                      value={String(draft.maxSubmissionsPerHour)}
                    />
                  </Field>
                </Grid>
                <Field htmlFor="form-description" label="Description">
                  <TextArea
                    id="form-description"
                    onChange={(value) =>
                      setDraft((current) => ({
                        ...current,
                        description: value,
                      }))
                    }
                    placeholder="Tell visitors why they should complete this form."
                    value={draft.description}
                  />
                </Field>
                <Field htmlFor="form-success" label="Success message">
                  <Input
                    id="form-success"
                    onChange={(value) =>
                      setDraft((current) => ({
                        ...current,
                        successMessage: value,
                      }))
                    }
                    required
                    value={draft.successMessage}
                  />
                </Field>

                <VStack alignItems="stretch" gap={3}>
                  <Flex alignItems="center" justifyContent="space-between">
                    <Text typography="subheading">Fields</Text>
                    <SecondaryButton
                      onClick={() =>
                        setDraft((current) => ({
                          ...current,
                          fields: [...current.fields, newDraftField()],
                        }))
                      }
                    >
                      Add field
                    </SecondaryButton>
                  </Flex>
                  {draft.fields.map((field, index) => (
                    <Box
                      key={`${field.id}-${index}`}
                      bg="$backgroundSecondary"
                      border="1px solid $border"
                      borderRadius="10px"
                      p={4}
                    >
                      <VStack alignItems="stretch" gap={4}>
                        <Grid
                          gap={3}
                          gridTemplateColumns={['1fr', 'repeat(3, 1fr)']}
                        >
                          <Field htmlFor={`field-id-${index}`} label="Field id">
                            <Input
                              id={`field-id-${index}`}
                              onChange={(value) =>
                                updateField(index, { id: value })
                              }
                              placeholder="email"
                              required
                              value={field.id}
                            />
                          </Field>
                          <Field htmlFor={`field-label-${index}`} label="Label">
                            <Input
                              id={`field-label-${index}`}
                              onChange={(value) =>
                                updateField(index, { label: value })
                              }
                              placeholder="Email address"
                              required
                              value={field.label}
                            />
                          </Field>
                          <Field htmlFor={`field-kind-${index}`} label="Type">
                            <Select
                              id={`field-kind-${index}`}
                              onChange={(value) =>
                                updateField(index, {
                                  kind: value as FieldKind,
                                  options:
                                    value === 'select' ? field.options : '',
                                })
                              }
                              value={field.kind}
                            >
                              <option value="text">Short text</option>
                              <option value="email">Email</option>
                              <option value="textarea">Long text</option>
                              <option value="checkbox">Checkbox</option>
                              <option value="select">Select</option>
                            </Select>
                          </Field>
                        </Grid>
                        {field.kind !== 'checkbox' ? (
                          <Field
                            htmlFor={`field-placeholder-${index}`}
                            label="Placeholder"
                          >
                            <Input
                              id={`field-placeholder-${index}`}
                              onChange={(value) =>
                                updateField(index, { placeholder: value })
                              }
                              value={field.placeholder}
                            />
                          </Field>
                        ) : null}
                        {field.kind === 'select' ? (
                          <Field
                            hint="One allowed option per line."
                            htmlFor={`field-options-${index}`}
                            label="Options"
                          >
                            <TextArea
                              id={`field-options-${index}`}
                              onChange={(value) =>
                                updateField(index, { options: value })
                              }
                              required
                              value={field.options}
                            />
                          </Field>
                        ) : null}
                        <Flex
                          alignItems="center"
                          gap={3}
                          justifyContent="space-between"
                        >
                          <Checkbox
                            checked={field.required}
                            label="Required"
                            onChange={(required) =>
                              updateField(index, { required })
                            }
                          />
                          <DangerButton
                            disabled={draft.fields.length === 1}
                            onClick={() =>
                              setDraft((current) => ({
                                ...current,
                                fields: current.fields.filter(
                                  (_, fieldIndex) => fieldIndex !== index,
                                ),
                              }))
                            }
                          >
                            Remove field
                          </DangerButton>
                        </Flex>
                      </VStack>
                    </Box>
                  ))}
                </VStack>

                <Flex
                  alignItems="center"
                  gap={4}
                  justifyContent="space-between"
                >
                  <Checkbox
                    checked={draft.enabled}
                    label="Allow public submissions"
                    onChange={(enabled) =>
                      setDraft((current) => ({ ...current, enabled }))
                    }
                  />
                  <PrimaryButton disabled={saving} type="submit">
                    {saving
                      ? 'Saving...'
                      : editingId === ''
                        ? 'Create form'
                        : 'Save changes'}
                  </PrimaryButton>
                </Flex>
              </VStack>
            </Box>
          </Box>
        ) : null}

        {!forbidden ? (
          <VStack alignItems="stretch" gap={3}>
            <Text typography="subheading">Configured forms</Text>
            {loading && forms.length === 0 ? (
              <EmptyCard>Loading forms...</EmptyCard>
            ) : forms.length === 0 ? (
              <EmptyCard>
                Create a form to open a validated public submission endpoint.
              </EmptyCard>
            ) : (
              <VStack alignItems="stretch" gap={4}>
                {forms.map((form) => (
                  <Box
                    key={form.id}
                    bg="$background"
                    border="1px solid $border"
                    borderRadius="12px"
                    p={5}
                  >
                    <VStack alignItems="stretch" gap={4}>
                      <Flex
                        alignItems={['stretch', null, 'flex-start']}
                        flexDirection={['column', null, 'row']}
                        gap={4}
                        justifyContent="space-between"
                      >
                        <VStack alignItems="flex-start" flex={1} gap={1}>
                          <Flex alignItems="center" flexWrap="wrap" gap={2}>
                            <Text typography="subheading">{form.name}</Text>
                            <StatusBadge enabled={form.enabled} />
                          </Flex>
                          {form.description !== '' ? (
                            <Text color="$textSecondary" typography="body">
                              {form.description}
                            </Text>
                          ) : null}
                          <Text color="$textTertiary" typography="label">
                            {form.fields.length} fields ·{' '}
                            {form.maxSubmissionsPerHour} submissions/hour ·
                            updated {formatDate(form.updatedAt)}
                          </Text>
                        </VStack>
                        <Flex flexWrap="wrap" gap={2}>
                          <SecondaryButton
                            onClick={() => void copyPublicId(form.id)}
                          >
                            Copy public id
                          </SecondaryButton>
                          <SecondaryButton onClick={() => beginEdit(form)}>
                            Edit
                          </SecondaryButton>
                          <SecondaryButton
                            disabled={busyId === form.id}
                            onClick={() => void toggleSubmissions(form)}
                          >
                            {busyId === form.id
                              ? 'Loading...'
                              : submissions[form.id] === undefined
                                ? 'Submissions'
                                : 'Hide inbox'}
                          </SecondaryButton>
                          <DangerButton
                            disabled={busyId === form.id}
                            onClick={() => void remove(form)}
                          >
                            Delete
                          </DangerButton>
                        </Flex>
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
                        GET /api/forms/public?id={form.id}
                      </Box>
                      {submissions[form.id] !== undefined ? (
                        <SubmissionInbox page={submissions[form.id]} />
                      ) : null}
                    </VStack>
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

function SubmissionInbox({ page }: { page: SubmissionPage }) {
  if (page.submissions.length === 0) {
    return <EmptyCard>No submissions yet.</EmptyCard>
  }
  return (
    <VStack alignItems="stretch" gap={3}>
      <Text color="$textSecondary" typography="label">
        {page.total} submissions
      </Text>
      {page.submissions.map((submission) => {
        const labels = new Map(
          submission.fields.map((field) => [field.id, field.label]),
        )
        return (
          <Box
            key={submission.id}
            bg="$backgroundSecondary"
            border="1px solid $border"
            borderRadius="8px"
            p={4}
          >
            <VStack alignItems="stretch" gap={2}>
              <Text color="$textTertiary" typography="label">
                {formatDate(submission.createdAt)}
              </Text>
              {Object.entries(submission.values).map(([key, value]) => (
                <Flex key={key} alignItems="flex-start" gap={3}>
                  <Text color="$textSecondary" minW="140px" typography="label">
                    {labels.get(key) ?? key}
                  </Text>
                  <Text typography="body">{displayValue(value)}</Text>
                </Flex>
              ))}
            </VStack>
          </Box>
        )
      })}
    </VStack>
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

function TextArea({
  onChange,
  ...props
}: Omit<React.TextareaHTMLAttributes<HTMLTextAreaElement>, 'onChange'> & {
  onChange: (value: string) => void
}) {
  return (
    <Box
      {...props}
      _focus={{ borderColor: '$primary' }}
      as="textarea"
      bg="$background"
      border="1px solid $border"
      borderRadius="8px"
      color="$text"
      minH="88px"
      onChange={(event: React.ChangeEvent<HTMLTextAreaElement>) =>
        onChange(event.target.value)
      }
      outline="none"
      p={3}
      resize="vertical"
    />
  )
}

function Select({
  children,
  onChange,
  ...props
}: Omit<React.SelectHTMLAttributes<HTMLSelectElement>, 'onChange'> & {
  children: React.ReactNode
  onChange: (value: string) => void
}) {
  return (
    <Box
      {...props}
      _focus={{ borderColor: '$primary' }}
      as="select"
      bg="$background"
      border="1px solid $border"
      borderRadius="8px"
      color="$text"
      onChange={(event: React.ChangeEvent<HTMLSelectElement>) =>
        onChange(event.target.value)
      }
      outline="none"
      p={3}
    >
      {children}
    </Box>
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

function StatusBadge({ enabled }: { enabled: boolean }) {
  return (
    <Box
      bg={enabled ? '$successLight' : '$warningLight'}
      borderRadius="999px"
      color={enabled ? '$success' : '$warning'}
      px={2}
      py={1}
    >
      <Text typography="label">{enabled ? 'Public' : 'Paused'}</Text>
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

function EmptyCard({ children }: { children: React.ReactNode }) {
  return (
    <Box bg="$background" border="1px dashed $border" borderRadius="12px" p={7}>
      <Text color="$textSecondary" textAlign="center" typography="body">
        {children}
      </Text>
    </Box>
  )
}
