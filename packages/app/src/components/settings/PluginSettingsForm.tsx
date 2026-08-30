'use client'

import { Box, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

export interface SettingsSchema {
  description?: string
  enum?: Array<string | number>
  format?: string
  properties?: Record<string, SettingsSchema>
  required?: string[]
  title?: string
  type?: string
}

interface PluginSettingsFormProps {
  apiPath: string
  defaultValue: Record<string, unknown>
  pluginName: string
  schema: SettingsSchema
}

function fieldLabel(name: string, schema: SettingsSchema) {
  if (schema.title) return schema.title
  return name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replaceAll('_', ' ')
    .replace(/^./, (letter) => letter.toUpperCase())
}

export function PluginSettingsForm({
  apiPath,
  defaultValue,
  pluginName,
  schema,
}: PluginSettingsFormProps) {
  const [value, setValue] = useState<Record<string, unknown>>(defaultValue)
  const [status, setStatus] = useState('Loading settings…')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    const controller = new AbortController()

    void fetch(apiPath, { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error('Could not load settings')
        setValue(await response.json())
        setStatus('')
      })
      .catch((error: unknown) => {
        if (error instanceof Error && error.name !== 'AbortError') {
          setStatus(error.message)
        }
      })

    return () => controller.abort()
  }, [apiPath])

  function update(name: string, nextValue: unknown) {
    setValue((current) => ({ ...current, [name]: nextValue }))
  }

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setStatus('')

    try {
      const response = await fetch(apiPath, {
        body: JSON.stringify(value),
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
      })
      if (!response.ok) {
        const body = (await response.json().catch(() => null)) as {
          error?: string
        } | null
        throw new Error(body?.error ?? 'Could not save settings')
      }
      setValue(await response.json())
      setStatus('Settings saved')
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : 'Could not save settings',
      )
    } finally {
      setSaving(false)
    }
  }

  const properties = schema.properties ?? {}

  return (
    <Box p={6}>
      <VStack alignItems="stretch" gap={6} maxW="720px">
        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">{pluginName} settings</Text>
          <Text color="$textSecondary" typography="body">
            {schema.description ?? 'Configure this plugin.'}
          </Text>
        </VStack>

        <Box
          as="form"
          bg="$background"
          border="1px solid $border"
          borderRadius="12px"
          onSubmit={save}
          p={6}
        >
          <VStack alignItems="stretch" gap={5}>
            {Object.entries(properties).map(([name, field]) => {
              const id = `setting-${name}`
              const label = fieldLabel(name, field)
              const current = value[name]

              if (field.type === 'boolean') {
                return (
                  <Box
                    key={name}
                    alignItems="center"
                    as="label"
                    display="flex"
                    gap={3}
                  >
                    <Box
                      as="input"
                      checked={Boolean(current)}
                      id={id}
                      onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                        update(name, event.target.checked)
                      }
                      type="checkbox"
                    />
                    <Text typography="body">{label}</Text>
                  </Box>
                )
              }

              return (
                <VStack key={name} alignItems="stretch" gap={2}>
                  <Text as="label" htmlFor={id} typography="label">
                    {label}
                  </Text>
                  {field.enum ? (
                    <Box
                      as="select"
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="8px"
                      color="$text"
                      id={id}
                      onChange={(event: React.ChangeEvent<HTMLSelectElement>) =>
                        update(name, event.target.value)
                      }
                      p={3}
                      value={String(current ?? '')}
                    >
                      {field.enum.map((option) => (
                        <option key={String(option)} value={option}>
                          {option}
                        </option>
                      ))}
                    </Box>
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
                        update(
                          name,
                          field.type === 'integer' || field.type === 'number'
                            ? event.target.valueAsNumber
                            : event.target.value,
                        )
                      }
                      outline="none"
                      p={3}
                      required={schema.required?.includes(name)}
                      type={
                        field.type === 'integer' || field.type === 'number'
                          ? 'number'
                          : 'text'
                      }
                      value={String(current ?? '')}
                    />
                  )}
                  {field.description ? (
                    <Text color="$textSecondary" typography="label">
                      {field.description}
                    </Text>
                  ) : null}
                </VStack>
              )
            })}

            <Box
              _hover={{ bg: '$primaryHover' }}
              as="button"
              bg="$primary"
              border="none"
              borderRadius="8px"
              color="white"
              cursor={saving ? 'not-allowed' : 'pointer'}
              disabled={saving}
              p={3}
              type="submit"
            >
              {saving ? 'Saving…' : 'Save settings'}
            </Box>
            <Text
              aria-live="polite"
              color="$textSecondary"
              minH="21px"
              typography="body"
            >
              {status}
            </Text>
          </VStack>
        </Box>
      </VStack>
    </Box>
  )
}
