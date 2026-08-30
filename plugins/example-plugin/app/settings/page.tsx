'use client'

import { Box, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

interface ExampleSettings {
  homepageMessage: string
  maintenanceMode: boolean
}

const initialSettings: ExampleSettings = {
  homepageMessage: '',
  maintenanceMode: false,
}

export default function ExampleSettingsPage() {
  const [settings, setSettings] = useState(initialSettings)
  const [status, setStatus] = useState('Loading settings…')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    const controller = new AbortController()

    void fetch('/api/example-plugin/settings', { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error('Could not load settings')
        setSettings(await response.json())
        setStatus('')
      })
      .catch((error: unknown) => {
        if (error instanceof Error && error.name !== 'AbortError') {
          setStatus(error.message)
        }
      })

    return () => controller.abort()
  }, [])

  async function save(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault()
    setSaving(true)
    setStatus('')

    try {
      const response = await fetch('/api/example-plugin/settings', {
        body: JSON.stringify(settings),
        headers: { 'Content-Type': 'application/json' },
        method: 'PUT',
      })
      if (!response.ok) throw new Error('Could not save settings')
      setSettings(await response.json())
      setStatus('Settings saved')
    } catch (error) {
      setStatus(
        error instanceof Error ? error.message : 'Could not save settings',
      )
    } finally {
      setSaving(false)
    }
  }

  return (
    <Box p={6}>
      <VStack alignItems="stretch" gap={6} maxW="720px">
        <VStack alignItems="flex-start" gap={2}>
          <Text typography="heading">Example plugin settings</Text>
          <Text color="$textSecondary" typography="body">
            This tailored screen overrides Yeollin&apos;s generated settings
            form.
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
            <VStack alignItems="stretch" gap={2}>
              <Text as="label" htmlFor="homepage-message" typography="label">
                Homepage message
              </Text>
              <Box
                _focus={{ borderColor: '$primary' }}
                as="input"
                bg="$background"
                border="1px solid $border"
                borderRadius="8px"
                color="$text"
                id="homepage-message"
                onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                  setSettings((current) => ({
                    ...current,
                    homepageMessage: event.target.value,
                  }))
                }
                outline="none"
                p={3}
                value={settings.homepageMessage}
              />
            </VStack>

            <Box alignItems="center" as="label" display="flex" gap={3}>
              <Box
                as="input"
                checked={settings.maintenanceMode}
                onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                  setSettings((current) => ({
                    ...current,
                    maintenanceMode: event.target.checked,
                  }))
                }
                type="checkbox"
              />
              <Text typography="body">Enable maintenance mode</Text>
            </Box>

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
