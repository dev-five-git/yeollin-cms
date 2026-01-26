'use client'

import { useCallback, useEffect, useState } from 'react'
import { Box, Flex, Text, VStack } from '@devup-ui/react'

interface Memo {
  id: number
  title: string
  content: string
  created_at: string
  updated_at: string
}

interface MemoForm {
  title: string
  content: string
}

export default function MemoListPage() {
  const [memos, setMemos] = useState<Memo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [isFormOpen, setIsFormOpen] = useState(false)
  const [editingMemo, setEditingMemo] = useState<Memo | null>(null)
  const [formData, setFormData] = useState<MemoForm>({ title: '', content: '' })
  const [submitting, setSubmitting] = useState(false)

  const fetchMemos = useCallback(async () => {
    setLoading(true)
    setError('')
    try {
      const response = await fetch('/memo')
      if (!response.ok) {
        throw new Error('Failed to fetch memos')
      }
      const data = await response.json()
      setMemos(data.memos || [])
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchMemos()
  }, [fetchMemos])

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
      const url = editingMemo ? `/memo/${editingMemo.id}` : '/memo'
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
      const response = await fetch(`/memo/${id}`, { method: 'DELETE' })
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
      minH="100vh"
      alignItems="flex-start"
      justifyContent="center"
      bg="$backgroundSecondary"
      py={8}
      px={4}
    >
      <Box
        bg="$background"
        p={8}
        borderRadius="16px"
        border="1px solid $border"
        w="100%"
        maxW="800px"
        boxShadow="0 4px 24px rgba(0, 0, 0, 0.1)"
      >
        <VStack gap={6} alignItems="stretch">
          <Flex justifyContent="space-between" alignItems="center">
            <VStack gap={1} alignItems="flex-start">
              <Text typography="heading">Memos</Text>
              <Text typography="body" color="$textSecondary">
                Manage your memos
              </Text>
            </VStack>
            <Box
              as="button"
              type="button"
              onClick={() => handleOpenForm()}
              p={3}
              px={4}
              borderRadius="8px"
              border="none"
              bg="$primary"
              color="white"
              fontSize="14px"
              fontWeight="600"
              cursor="pointer"
              transition="all 0.2s ease"
              _hover={{ bg: '$primaryHover' }}
              _active={{ transform: 'scale(0.98)' }}
            >
              Add Memo
            </Box>
          </Flex>

          {error && (
            <Box
              bg="$errorLight"
              p={3}
              borderRadius="8px"
              border="1px solid $error"
            >
              <Text typography="body" color="$error">
                {error}
              </Text>
            </Box>
          )}

          {isFormOpen && (
            <Box
              bg="$backgroundSecondary"
              p={6}
              borderRadius="12px"
              border="1px solid $border"
            >
              <form onSubmit={handleSubmit}>
                <VStack gap={4} alignItems="stretch">
                  <Text typography="subheading">
                    {editingMemo ? 'Edit Memo' : 'New Memo'}
                  </Text>

                  <VStack gap={2} alignItems="stretch">
                    <Text as="label" typography="label" htmlFor="memo-title">
                      Title
                    </Text>
                    <Box
                      as="input"
                      id="memo-title"
                      type="text"
                      value={formData.title}
                      onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
                        setFormData((prev) => ({ ...prev, title: e.target.value }))
                      }
                      required
                      p={3}
                      borderRadius="8px"
                      border="1px solid $border"
                      bg="$background"
                      color="$text"
                      fontSize="16px"
                      outline="none"
                      _focus={{ borderColor: '$primary' }}
                      _placeholder={{ color: '$textTertiary' }}
                      placeholder="Enter memo title"
                    />
                  </VStack>

                  <VStack gap={2} alignItems="stretch">
                    <Text as="label" typography="label" htmlFor="memo-content">
                      Content
                    </Text>
                    <Box
                      as="textarea"
                      id="memo-content"
                      value={formData.content}
                      onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) =>
                        setFormData((prev) => ({ ...prev, content: e.target.value }))
                      }
                      required
                      p={3}
                      borderRadius="8px"
                      border="1px solid $border"
                      bg="$background"
                      color="$text"
                      fontSize="16px"
                      outline="none"
                      minH="120px"
                      resize="vertical"
                      _focus={{ borderColor: '$primary' }}
                      _placeholder={{ color: '$textTertiary' }}
                      placeholder="Enter memo content"
                    />
                  </VStack>

                  <Flex gap={3} justifyContent="flex-end">
                    <Box
                      as="button"
                      type="button"
                      onClick={handleCloseForm}
                      p={3}
                      px={4}
                      borderRadius="8px"
                      border="1px solid $border"
                      bg="$background"
                      color="$text"
                      fontSize="14px"
                      fontWeight="500"
                      cursor="pointer"
                      transition="all 0.2s ease"
                      _hover={{ bg: '$backgroundSecondary' }}
                    >
                      Cancel
                    </Box>
                    <Box
                      as="button"
                      type="submit"
                      disabled={submitting}
                      p={3}
                      px={4}
                      borderRadius="8px"
                      border="none"
                      bg="$primary"
                      color="white"
                      fontSize="14px"
                      fontWeight="600"
                      cursor={submitting ? 'not-allowed' : 'pointer'}
                      opacity={submitting ? 0.7 : 1}
                      transition="all 0.2s ease"
                      _hover={{ bg: '$primaryHover' }}
                      _active={{ transform: 'scale(0.98)' }}
                    >
                      {submitting ? 'Saving...' : editingMemo ? 'Update' : 'Create'}
                    </Box>
                  </Flex>
                </VStack>
              </form>
            </Box>
          )}

          {loading ? (
            <Flex justifyContent="center" py={8}>
              <Text typography="body" color="$textSecondary">
                Loading memos...
              </Text>
            </Flex>
          ) : memos.length === 0 ? (
            <Flex
              justifyContent="center"
              alignItems="center"
              py={8}
              borderRadius="12px"
              border="1px dashed $border"
            >
              <VStack gap={2} alignItems="center">
                <Text typography="body" color="$textSecondary">
                  No memos yet
                </Text>
                <Text typography="label" color="$textTertiary">
                  Click "Add Memo" to create your first memo
                </Text>
              </VStack>
            </Flex>
          ) : (
            <VStack gap={3} alignItems="stretch">
              {memos.map((memo) => (
                <Box
                  key={memo.id}
                  bg="$backgroundSecondary"
                  p={4}
                  borderRadius="12px"
                  border="1px solid $border"
                  transition="all 0.2s ease"
                  _hover={{ borderColor: '$primary', boxShadow: '0 2px 8px rgba(0, 0, 0, 0.05)' }}
                >
                  <Flex justifyContent="space-between" alignItems="flex-start" gap={4}>
                    <VStack gap={2} alignItems="flex-start" flex={1}>
                      <Text typography="subheading" color="$text">
                        {memo.title}
                      </Text>
                      <Text
                        typography="body"
                        color="$textSecondary"
                        overflow="hidden"
                        textOverflow="ellipsis"
                        whiteSpace="nowrap"
                        maxW="100%"
                      >
                        {memo.content.length > 100
                          ? `${memo.content.substring(0, 100)}...`
                          : memo.content}
                      </Text>
                      <Text typography="label" color="$textTertiary">
                        {new Date(memo.updated_at).toLocaleDateString()}
                      </Text>
                    </VStack>
                    <Flex gap={2}>
                      <Box
                        as="button"
                        type="button"
                        onClick={() => handleOpenForm(memo)}
                        p={2}
                        px={3}
                        borderRadius="6px"
                        border="1px solid $border"
                        bg="$background"
                        color="$text"
                        fontSize="13px"
                        fontWeight="500"
                        cursor="pointer"
                        transition="all 0.2s ease"
                        _hover={{ bg: '$backgroundSecondary', borderColor: '$primary' }}
                      >
                        Edit
                      </Box>
                      <Box
                        as="button"
                        type="button"
                        onClick={() => handleDelete(memo.id)}
                        p={2}
                        px={3}
                        borderRadius="6px"
                        border="1px solid $error"
                        bg="$background"
                        color="$error"
                        fontSize="13px"
                        fontWeight="500"
                        cursor="pointer"
                        transition="all 0.2s ease"
                        _hover={{ bg: '$errorLight' }}
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
