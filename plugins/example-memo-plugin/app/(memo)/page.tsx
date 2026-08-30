'use client'

import { Box, Flex, Text, VStack } from '@devup-ui/react'
import { useCallback, useEffect, useState } from 'react'

interface Memo {
  id: number
  title: string
  content: string
  createdAt: string
  updatedAt: string
}

interface MemoForm {
  title: string
  content: string
}

async function fetchMemoList(): Promise<Memo[]> {
  const response = await fetch('/api/example-memo-plugin')
  if (!response.ok) {
    throw new Error('Failed to fetch memos')
  }
  const data = await response.json()
  return data.memos || []
}

export default function MemoListPage() {
  const [memos, setMemos] = useState<Memo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [isFormOpen, setIsFormOpen] = useState(false)
  const [editingMemo, setEditingMemo] = useState<Memo | null>(null)
  const [formData, setFormData] = useState<MemoForm>({ title: '', content: '' })
  const [submitting, setSubmitting] = useState(false)

  // Refetch after a mutation: resets the list state before loading again.
  const fetchMemos = useCallback(async () => {
    setLoading(true)
    setError('')
    try {
      setMemos(await fetchMemoList())
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred')
    } finally {
      setLoading(false)
    }
  }, [])

  // Initial load. Runs inline so no state is set synchronously during the
  // effect, and drops its result if the component unmounts mid-flight.
  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const memos = await fetchMemoList()
        if (!cancelled) setMemos(memos)
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'An error occurred')
        }
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  const handleOpenForm = (memo?: Memo) => {
    if (memo) {
      setEditingMemo(memo)
      setFormData({ title: memo.title, content: memo.content })
    } else {
      setEditingMemo(null)
      setFormData({ title: '', content: '' })
    }
    setIsFormOpen(true)
  }

  const handleCloseForm = () => {
    setIsFormOpen(false)
    setEditingMemo(null)
    setFormData({ title: '', content: '' })
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setSubmitting(true)
    setError('')

    try {
      const url = editingMemo
        ? `/api/example-memo-plugin/${editingMemo.id}`
        : '/api/example-memo-plugin'
      const method = editingMemo ? 'PATCH' : 'POST'

      const response = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(formData),
      })

      if (!response.ok) {
        const data = await response.json()
        throw new Error(data.error || 'Failed to save memo')
      }

      handleCloseForm()
      await fetchMemos()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred')
    } finally {
      setSubmitting(false)
    }
  }

  const handleDelete = async (id: number) => {
    if (!confirm('Are you sure you want to delete this memo?')) {
      return
    }

    try {
      const response = await fetch(`/api/example-memo-plugin/${id}`, {
        method: 'DELETE',
      })
      if (!response.ok) {
        const data = await response.json()
        throw new Error(data.error || 'Failed to delete memo')
      }
      await fetchMemos()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred')
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
      <Box
        bg="$background"
        border="1px solid $border"
        borderRadius="16px"
        boxShadow="0 4px 24px rgba(0, 0, 0, 0.1)"
        maxW="800px"
        p={8}
        w="100%"
      >
        <VStack alignItems="stretch" gap={6}>
          <Flex alignItems="center" justifyContent="space-between">
            <VStack alignItems="flex-start" gap={1}>
              <Text typography="heading">Memos</Text>
              <Text color="$textSecondary" typography="body">
                Manage your memos
              </Text>
            </VStack>
            <Box
              _active={{ transform: 'scale(0.98)' }}
              _hover={{ bg: '$primaryHover' }}
              as="button"
              bg="$primary"
              border="none"
              borderRadius="8px"
              color="white"
              cursor="pointer"
              fontSize="14px"
              fontWeight="600"
              onClick={() => handleOpenForm()}
              p={3}
              px={4}
              transition="all 0.2s ease"
              type="button"
            >
              Add Memo
            </Box>
          </Flex>

          {error && (
            <Box
              bg="$errorLight"
              border="1px solid $error"
              borderRadius="8px"
              p={3}
            >
              <Text color="$error" typography="body">
                {error}
              </Text>
            </Box>
          )}

          {isFormOpen && (
            <Box
              bg="$backgroundSecondary"
              border="1px solid $border"
              borderRadius="12px"
              p={6}
            >
              <form onSubmit={handleSubmit}>
                <VStack alignItems="stretch" gap={4}>
                  <Text typography="subheading">
                    {editingMemo ? 'Edit Memo' : 'New Memo'}
                  </Text>

                  <VStack alignItems="stretch" gap={2}>
                    <Text as="label" htmlFor="memo-title" typography="label">
                      Title
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
                      id="memo-title"
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        setFormData((prev) => ({
                          ...prev,
                          title: e.target.value,
                        }))
                      }
                      outline="none"
                      p={3}
                      placeholder="Enter memo title"
                      required
                      type="text"
                      value={formData.title}
                    />
                  </VStack>

                  <VStack alignItems="stretch" gap={2}>
                    <Text as="label" htmlFor="memo-content" typography="label">
                      Content
                    </Text>
                    <Box
                      _focus={{ borderColor: '$primary' }}
                      _placeholder={{ color: '$textTertiary' }}
                      as="textarea"
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="8px"
                      color="$text"
                      fontSize="16px"
                      id="memo-content"
                      minH="120px"
                      onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
                        setFormData((prev) => ({
                          ...prev,
                          content: e.target.value,
                        }))
                      }
                      outline="none"
                      p={3}
                      placeholder="Enter memo content"
                      required
                      resize="vertical"
                      value={formData.content}
                    />
                  </VStack>

                  <Flex gap={3} justifyContent="flex-end">
                    <Box
                      _hover={{ bg: '$backgroundSecondary' }}
                      as="button"
                      bg="$background"
                      border="1px solid $border"
                      borderRadius="8px"
                      color="$text"
                      cursor="pointer"
                      fontSize="14px"
                      fontWeight="500"
                      onClick={handleCloseForm}
                      p={3}
                      px={4}
                      transition="all 0.2s ease"
                      type="button"
                    >
                      Cancel
                    </Box>
                    <Box
                      _active={{ transform: 'scale(0.98)' }}
                      _hover={{ bg: '$primaryHover' }}
                      as="button"
                      bg="$primary"
                      border="none"
                      borderRadius="8px"
                      color="white"
                      cursor={submitting ? 'not-allowed' : 'pointer'}
                      disabled={submitting}
                      fontSize="14px"
                      fontWeight="600"
                      opacity={submitting ? 0.7 : 1}
                      p={3}
                      px={4}
                      transition="all 0.2s ease"
                      type="submit"
                    >
                      {submitting
                        ? 'Saving...'
                        : editingMemo
                          ? 'Update'
                          : 'Create'}
                    </Box>
                  </Flex>
                </VStack>
              </form>
            </Box>
          )}

          {loading ? (
            <Flex justifyContent="center" py={8}>
              <Text color="$textSecondary" typography="body">
                Loading memos...
              </Text>
            </Flex>
          ) : memos.length === 0 ? (
            <Flex
              alignItems="center"
              border="1px dashed $border"
              borderRadius="12px"
              justifyContent="center"
              py={8}
            >
              <VStack alignItems="center" gap={2}>
                <Text color="$textSecondary" typography="body">
                  No memos yet
                </Text>
                <Text color="$textTertiary" typography="label">
                  Click &quot;Add Memo&quot; to create your first memo
                </Text>
              </VStack>
            </Flex>
          ) : (
            <VStack alignItems="stretch" gap={3}>
              {memos.map((memo) => (
                <Box
                  key={memo.id}
                  _hover={{
                    borderColor: '$primary',
                    boxShadow: '0 2px 8px rgba(0, 0, 0, 0.05)',
                  }}
                  bg="$backgroundSecondary"
                  border="1px solid $border"
                  borderRadius="12px"
                  p={4}
                  transition="all 0.2s ease"
                >
                  <Flex
                    alignItems="flex-start"
                    gap={4}
                    justifyContent="space-between"
                  >
                    <VStack alignItems="flex-start" flex={1} gap={2}>
                      <Text color="$text" typography="subheading">
                        {memo.title}
                      </Text>
                      <Text
                        color="$textSecondary"
                        maxW="100%"
                        overflow="hidden"
                        textOverflow="ellipsis"
                        typography="body"
                        whiteSpace="nowrap"
                      >
                        {memo.content.length > 100
                          ? `${memo.content.substring(0, 100)}...`
                          : memo.content}
                      </Text>
                      <Text color="$textTertiary" typography="label">
                        {new Date(memo.updatedAt).toLocaleDateString()}
                      </Text>
                    </VStack>
                    <Flex gap={2}>
                      <Box
                        _hover={{
                          bg: '$backgroundSecondary',
                          borderColor: '$primary',
                        }}
                        as="button"
                        bg="$background"
                        border="1px solid $border"
                        borderRadius="6px"
                        color="$text"
                        cursor="pointer"
                        fontSize="13px"
                        fontWeight="500"
                        onClick={() => handleOpenForm(memo)}
                        p={2}
                        px={3}
                        transition="all 0.2s ease"
                        type="button"
                      >
                        Edit
                      </Box>
                      <Box
                        _hover={{ bg: '$errorLight' }}
                        as="button"
                        bg="$background"
                        border="1px solid $error"
                        borderRadius="6px"
                        color="$error"
                        cursor="pointer"
                        fontSize="13px"
                        fontWeight="500"
                        onClick={() => handleDelete(memo.id)}
                        p={2}
                        px={3}
                        transition="all 0.2s ease"
                        type="button"
                      >
                        Delete
                      </Box>
                    </Flex>
                  </Flex>
                </Box>
              ))}
            </VStack>
          )}
        </VStack>
      </Box>
    </Flex>
  )
}
