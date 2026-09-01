interface DemoState {
  auditEvents: Array<Record<string, unknown>>
  contentEntries: Array<Record<string, unknown>>
  deliveries: Array<Record<string, unknown>>
  forms: Array<Record<string, unknown>>
  media: Array<Record<string, unknown>>
  memos: Array<Record<string, unknown>>
  redirects: Array<Record<string, unknown>>
  settings: Record<string, Record<string, unknown>>
  submissions: Record<string, Array<Record<string, unknown>>>
  users: Array<Record<string, unknown>>
  webhooks: Array<Record<string, unknown>>
}

const DEMO_NOW = '2026-09-01T04:30:00.000Z'
const DEMO_BASE_PATH = import.meta.env.VITE_YEOLLIN_BASE_PATH ?? ''

let installed = false
let sequence = 100
let state = createDemoState()

function id(prefix: string): string {
  sequence += 1
  return `${prefix}-${sequence}`
}

function imageData(label: string, start: string, end: string): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="960" height="640" viewBox="0 0 960 640"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${start}"/><stop offset="1" stop-color="${end}"/></linearGradient></defs><rect width="960" height="640" rx="48" fill="url(#g)"/><circle cx="760" cy="150" r="110" fill="white" opacity=".16"/><circle cx="170" cy="520" r="180" fill="white" opacity=".1"/><text x="64" y="560" fill="white" font-family="system-ui,sans-serif" font-size="54" font-weight="700">${label}</text></svg>`
  return `data:image/svg+xml,${encodeURIComponent(svg)}`
}

function createDemoState(): DemoState {
  const formFields = [
    {
      id: 'email',
      kind: 'email',
      label: 'Work email',
      options: [],
      placeholder: 'you@example.com',
      required: true,
    },
    {
      id: 'message',
      kind: 'textarea',
      label: 'How can we help?',
      options: [],
      placeholder: 'Tell us about your project',
      required: true,
    },
  ]

  return {
    auditEvents: [
      {
        createdAt: '2026-09-01T04:18:00.000Z',
        id: 14,
        name: 'content.pages.published',
        payload: { actor: 'admin', slug: 'welcome-to-yeollin' },
      },
      {
        createdAt: '2026-09-01T03:42:00.000Z',
        id: 13,
        name: 'auth.user.role_changed',
        payload: { actor: 'admin', role: 'user', username: 'editor' },
      },
      {
        createdAt: '2026-08-31T23:10:00.000Z',
        id: 12,
        name: 'media.asset.created',
        payload: { mimeType: 'image/svg+xml', originalName: 'open-cms.svg' },
      },
    ],
    contentEntries: [
      {
        author: 'admin',
        collection: 'pages',
        createdAt: '2026-08-28T01:00:00.000Z',
        fields: {
          body: 'Yeollin CMS keeps backend routes, frontend pages, migrations, and metadata together in plugin crates.',
          excerpt: 'A quick tour of an open and extensible Rust CMS.',
          heroImage: 'media:11111111111111111111111111111111',
        },
        id: 'page-welcome',
        publishedAt: '2026-08-29T02:00:00.000Z',
        slug: 'welcome-to-yeollin',
        status: 'published',
        title: 'Welcome to Yeollin CMS',
        updatedAt: '2026-09-01T04:18:00.000Z',
      },
      {
        author: 'editor',
        collection: 'pages',
        createdAt: '2026-08-30T08:15:00.000Z',
        fields: {
          body: 'This draft demonstrates typed content fields and the publish workflow.',
          excerpt: 'An unpublished page ready for editorial review.',
          heroImage: null,
        },
        id: 'page-roadmap',
        publishedAt: null,
        slug: 'product-roadmap',
        status: 'draft',
        title: 'Product roadmap',
        updatedAt: '2026-09-01T01:12:00.000Z',
      },
    ],
    deliveries: [
      {
        attempts: 1,
        createdAt: '2026-09-01T04:18:01.000Z',
        deliveredAt: '2026-09-01T04:18:02.000Z',
        eventId: 14,
        eventName: 'content.pages.published',
        id: 'delivery-1',
        lastError: null,
        maxAttempts: 5,
        responseStatus: 204,
        status: 'delivered',
        updatedAt: '2026-09-01T04:18:02.000Z',
        webhookId: 'webhook-1',
      },
      {
        attempts: 3,
        createdAt: '2026-09-01T03:42:01.000Z',
        deliveredAt: null,
        eventId: 13,
        eventName: 'auth.user.role_changed',
        id: 'delivery-2',
        lastError: 'The demo endpoint returned HTTP 503.',
        maxAttempts: 5,
        responseStatus: 503,
        status: 'dead_letter',
        updatedAt: '2026-09-01T03:45:01.000Z',
        webhookId: 'webhook-1',
      },
    ],
    forms: [
      {
        createdAt: '2026-08-27T09:00:00.000Z',
        createdBy: 'admin',
        description: 'Collect questions from prospective Yeollin users.',
        enabled: true,
        fields: formFields,
        id: 'contact-form',
        maxSubmissionsPerHour: 100,
        name: 'Contact us',
        successMessage: 'Thanks - we will get back to you soon.',
        updatedAt: '2026-08-31T11:25:00.000Z',
      },
    ],
    media: [
      {
        createdAt: '2026-08-31T23:10:00.000Z',
        id: '11111111111111111111111111111111',
        mimeType: 'image/svg+xml',
        originalName: 'open-cms.svg',
        reference: 'media:11111111111111111111111111111111',
        sizeBytes: 18432,
        uploadedBy: 'admin',
        url: imageData('OPEN CMS', '#0070f3', '#7c3aed'),
      },
      {
        createdAt: '2026-08-30T07:40:00.000Z',
        id: '22222222222222222222222222222222',
        mimeType: 'image/svg+xml',
        originalName: 'plugin-workspace.svg',
        reference: 'media:22222222222222222222222222222222',
        sizeBytes: 22104,
        uploadedBy: 'editor',
        url: imageData('PLUGIN WORKSPACE', '#0f766e', '#06b6d4'),
      },
    ],
    memos: [
      {
        content:
          'The GitHub Pages demo runs entirely in the browser. Try creating, editing, and deleting this memo.',
        createdAt: '2026-09-01T02:10:00.000Z',
        id: 1,
        title: 'Welcome to the interactive demo',
        updatedAt: '2026-09-01T02:10:00.000Z',
      },
      {
        content:
          'Plugins package Axum routes and vinext UI together in one Rust crate.',
        createdAt: '2026-08-31T09:20:00.000Z',
        id: 2,
        title: 'Plugin architecture',
        updatedAt: '2026-08-31T09:20:00.000Z',
      },
    ],
    redirects: [
      {
        createdAt: '2026-08-29T04:00:00.000Z',
        createdBy: 'admin',
        destinationPath: '/welcome-to-yeollin',
        enabled: true,
        id: 'redirect-1',
        sourcePath: '/getting-started',
        updatedAt: '2026-08-29T04:00:00.000Z',
      },
      {
        createdAt: '2026-08-30T05:00:00.000Z',
        createdBy: 'admin',
        destinationPath: 'https://github.com/dev-five-git/yeollin-cms',
        enabled: false,
        id: 'redirect-2',
        sourcePath: '/source',
        updatedAt: '2026-08-31T05:00:00.000Z',
      },
    ],
    settings: {
      '/api/example-plugin/settings': {
        homepageMessage: 'Welcome to the Yeollin CMS demo',
        maintenanceMode: false,
      },
    },
    submissions: {
      'contact-form': [
        {
          createdAt: '2026-09-01T00:20:00.000Z',
          fields: formFields,
          formId: 'contact-form',
          formName: 'Contact us',
          id: 'submission-1',
          values: {
            email: 'hello@example.com',
            message: 'Can I build a multilingual publication with Yeollin?',
          },
        },
      ],
    },
    users: [
      {
        createdAt: '2026-08-24T01:00:00.000Z',
        id: 1,
        role: 'admin',
        username: 'admin',
      },
      {
        createdAt: '2026-08-28T06:30:00.000Z',
        id: 2,
        role: 'user',
        username: 'editor',
      },
    ],
    webhooks: [
      {
        allowPrivateNetworks: false,
        createdAt: '2026-08-26T03:00:00.000Z',
        enabled: true,
        eventNames: ['content.pages.published', 'auth.user.role_changed'],
        hasSecret: true,
        id: 'webhook-1',
        name: 'Editorial automation',
        timeoutSeconds: 5,
        updatedAt: '2026-08-31T08:20:00.000Z',
        url: 'https://example.com/hooks/yeollin',
      },
    ],
  }
}

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    headers: { 'Content-Type': 'application/json' },
    status,
  })
}

function bodyOf(init?: RequestInit): Record<string, unknown> {
  if (typeof init?.body !== 'string') return {}
  try {
    const value = JSON.parse(init.body) as unknown
    return typeof value === 'object' && value !== null
      ? (value as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

function pageOf<T>(items: T[], url: URL, defaultPageSize: number) {
  const page = Math.max(1, Number(url.searchParams.get('page') ?? 1))
  const pageSize = Math.max(
    1,
    Number(url.searchParams.get('pageSize') ?? defaultPageSize),
  )
  const start = (page - 1) * pageSize
  return { items: items.slice(start, start + pageSize), page, pageSize }
}

function handleUsers(path: string, method: string, init?: RequestInit) {
  if (path === '/api/auth/login' && method === 'POST') {
    return json({
      access_token: 'yeollin-demo-access-token',
      expires_in: 3600,
      refresh_token: 'yeollin-demo-refresh-token',
    })
  }
  if (path === '/api/auth/users' && method === 'GET') {
    return json({ total: state.users.length, users: state.users })
  }
  if (path === '/api/auth/users' && method === 'POST') {
    const body = bodyOf(init)
    state.users.push({
      createdAt: DEMO_NOW,
      id: sequence + 1,
      role: body.role === 'admin' ? 'admin' : 'user',
      username: String(body.username ?? 'new-user'),
    })
    sequence += 1
    return json(state.users.at(-1), 201)
  }
  const match = path.match(/^\/api\/auth\/users\/(\d+)(\/password)?$/)
  if (!match) return null
  const userId = Number(match[1])
  const user = state.users.find((entry) => entry.id === userId)
  if (!user) return json({ error: 'User not found.' }, 404)
  if (match[2] === '/password' && method === 'POST') return json({ ok: true })
  if (method === 'PATCH') Object.assign(user, bodyOf(init))
  if (method === 'DELETE')
    state.users = state.users.filter((item) => item !== user)
  return json(user)
}

function handleMemos(path: string, method: string, init?: RequestInit) {
  if (path === '/api/example-memo-plugin' && method === 'GET') {
    return json({ memos: state.memos })
  }
  if (path === '/api/example-memo-plugin' && method === 'POST') {
    const memo = {
      ...bodyOf(init),
      createdAt: DEMO_NOW,
      id: sequence + 1,
      updatedAt: DEMO_NOW,
    }
    sequence += 1
    state.memos.unshift(memo)
    return json(memo, 201)
  }
  const match = path.match(/^\/api\/example-memo-plugin\/(\d+)$/)
  if (!match) return null
  const memo = state.memos.find((entry) => entry.id === Number(match[1]))
  if (!memo) return json({ error: 'Memo not found.' }, 404)
  if (method === 'PATCH')
    Object.assign(memo, bodyOf(init), { updatedAt: DEMO_NOW })
  if (method === 'DELETE')
    state.memos = state.memos.filter((item) => item !== memo)
  return json(memo)
}

function handleRedirects(path: string, method: string, init?: RequestInit) {
  if (path === '/api/redirects' && method === 'GET') {
    return json({ redirects: state.redirects })
  }
  if (path === '/api/redirects' && method === 'POST') {
    const redirect = {
      ...bodyOf(init),
      createdAt: DEMO_NOW,
      createdBy: 'demo-admin',
      id: id('redirect'),
      updatedAt: DEMO_NOW,
    }
    state.redirects.unshift(redirect)
    return json(redirect, 201)
  }
  const match = path.match(/^\/api\/redirects\/([^/]+)$/)
  if (!match) return null
  const redirect = state.redirects.find((entry) => entry.id === match[1])
  if (!redirect) return json({ error: 'Redirect not found.' }, 404)
  if (method === 'PUT')
    Object.assign(redirect, bodyOf(init), { updatedAt: DEMO_NOW })
  if (method === 'DELETE') {
    state.redirects = state.redirects.filter((item) => item !== redirect)
  }
  return json(redirect)
}

function handleForms(path: string, method: string, init?: RequestInit) {
  if (path === '/api/forms' && method === 'GET')
    return json({ forms: state.forms })
  if (path === '/api/forms' && method === 'POST') {
    const form = {
      ...bodyOf(init),
      createdAt: DEMO_NOW,
      createdBy: 'demo-admin',
      id: id('form'),
      updatedAt: DEMO_NOW,
    }
    state.forms.unshift(form)
    return json(form, 201)
  }
  const submissions = path.match(/^\/api\/forms\/([^/]+)\/submissions$/)
  if (submissions && method === 'GET') {
    const items = state.submissions[submissions[1]] ?? []
    return json({
      page: 1,
      pageSize: 50,
      submissions: items,
      total: items.length,
    })
  }
  const match = path.match(/^\/api\/forms\/([^/]+)$/)
  if (!match) return null
  const form = state.forms.find((entry) => entry.id === match[1])
  if (!form) return json({ error: 'Form not found.' }, 404)
  if (method === 'PUT')
    Object.assign(form, bodyOf(init), { updatedAt: DEMO_NOW })
  if (method === 'DELETE') {
    state.forms = state.forms.filter((item) => item !== form)
    delete state.submissions[match[1]]
  }
  return json(form)
}

function handleMedia(
  path: string,
  method: string,
  init: RequestInit | undefined,
  url: URL,
) {
  if (path === '/api/media/settings' && method === 'GET') {
    return json({ maxUploadMegabytes: 5 })
  }
  if (path === '/api/media' && method === 'GET') {
    const page = pageOf(state.media, url, 24)
    return json({
      media: page.items,
      page: page.page,
      pageSize: page.pageSize,
      total: state.media.length,
    })
  }
  if (path === '/api/media' && method === 'POST') {
    const file = init?.body instanceof FormData ? init.body.get('file') : null
    const mediaId = String(sequence + 1).padStart(32, '0')
    sequence += 1
    const media = {
      createdAt: DEMO_NOW,
      id: mediaId,
      mimeType: file instanceof File ? file.type : 'image/svg+xml',
      originalName: file instanceof File ? file.name : 'demo-upload.svg',
      reference: `media:${mediaId}`,
      sizeBytes: file instanceof File ? file.size : 1024,
      uploadedBy: 'demo-admin',
      url:
        file instanceof File
          ? URL.createObjectURL(file)
          : imageData('DEMO UPLOAD', '#9333ea', '#db2777'),
    }
    state.media.unshift(media)
    return json(media, 201)
  }
  const match = path.match(/^\/api\/media\/([^/]+)$/)
  if (!match || method !== 'DELETE') return null
  state.media = state.media.filter((entry) => entry.id !== match[1])
  return json({ ok: true })
}

function handleAudit(path: string, method: string, url: URL) {
  if (path !== '/api/audit-log' || method !== 'GET') return null
  const eventName = url.searchParams.get('eventName') ?? ''
  const filtered =
    eventName === ''
      ? state.auditEvents
      : state.auditEvents.filter((event) => event.name === eventName)
  const page = pageOf(filtered, url, 20)
  return json({
    events: page.items,
    page: page.page,
    pageSize: page.pageSize,
    retentionDays: 90,
    total: filtered.length,
  })
}

function handleSearch(path: string, method: string, url: URL) {
  if (path !== '/api/search' || method !== 'GET') return null
  const query = (url.searchParams.get('q') ?? '').toLowerCase()
  const status = url.searchParams.get('status') ?? ''
  const results = state.contentEntries
    .filter((entry) => {
      const haystack =
        `${entry.title} ${entry.slug} ${JSON.stringify(entry.fields)}`.toLowerCase()
      return (
        haystack.includes(query) && (status === '' || entry.status === status)
      )
    })
    .map((entry) => ({
      collection: entry.collection,
      excerpt: (entry.fields as Record<string, unknown>).excerpt ?? '',
      id: entry.id,
      relevance: 1,
      status: entry.status,
      subject: `${entry.collection}:${entry.id}`,
      title: entry.title,
      updatedAt: entry.updatedAt,
      url: '/content/pages',
    }))
  const page = pageOf(results, url, 20)
  return json({
    page: page.page,
    pageSize: page.pageSize,
    query: url.searchParams.get('q') ?? '',
    results: page.items,
    total: results.length,
  })
}

function handleWebhooks(
  path: string,
  method: string,
  init: RequestInit | undefined,
  url: URL,
) {
  if (path === '/api/webhooks' && method === 'GET') {
    return json({ webhooks: state.webhooks })
  }
  if (path === '/api/webhooks' && method === 'POST') {
    const webhook = {
      ...bodyOf(init),
      createdAt: DEMO_NOW,
      hasSecret: true,
      id: id('webhook'),
      updatedAt: DEMO_NOW,
    }
    state.webhooks.unshift(webhook)
    return json(webhook, 201)
  }
  if (path === '/api/webhooks/deliveries' && method === 'GET') {
    const status = url.searchParams.get('status') ?? ''
    const filtered =
      status === ''
        ? state.deliveries
        : state.deliveries.filter((delivery) => delivery.status === status)
    const page = pageOf(filtered, url, 25)
    return json({
      deliveries: page.items,
      page: page.page,
      pageSize: page.pageSize,
      total: filtered.length,
    })
  }
  const retry = path.match(/^\/api\/webhooks\/deliveries\/([^/]+)\/retry$/)
  if (retry && method === 'POST') {
    const delivery = state.deliveries.find((entry) => entry.id === retry[1])
    if (!delivery) return json({ error: 'Delivery not found.' }, 404)
    Object.assign(delivery, {
      attempts: 0,
      lastError: null,
      responseStatus: null,
      status: 'pending',
      updatedAt: DEMO_NOW,
    })
    return json(delivery)
  }
  const match = path.match(/^\/api\/webhooks\/([^/]+)$/)
  if (!match) return null
  const webhook = state.webhooks.find((entry) => entry.id === match[1])
  if (!webhook) return json({ error: 'Webhook not found.' }, 404)
  if (method === 'PUT')
    Object.assign(webhook, bodyOf(init), {
      hasSecret: true,
      updatedAt: DEMO_NOW,
    })
  if (method === 'DELETE') {
    state.webhooks = state.webhooks.filter((item) => item !== webhook)
    state.deliveries = state.deliveries.filter(
      (item) => item.webhookId !== match[1],
    )
  }
  return json(webhook)
}

function handleContent(
  path: string,
  method: string,
  init: RequestInit | undefined,
  url: URL,
) {
  const root = '/api/content/pages'
  if (path === root && method === 'GET') {
    const status = url.searchParams.get('status') ?? ''
    const filtered =
      status === '' || status === 'all'
        ? state.contentEntries
        : state.contentEntries.filter((entry) => entry.status === status)
    const page = pageOf(filtered, url, 20)
    return json({
      entries: page.items,
      page: page.page,
      pageSize: page.pageSize,
      total: filtered.length,
    })
  }
  if (path === root && method === 'POST') {
    const entry = {
      ...bodyOf(init),
      author: 'demo-admin',
      collection: 'pages',
      createdAt: DEMO_NOW,
      id: id('page'),
      publishedAt: null,
      status: 'draft',
      updatedAt: DEMO_NOW,
    }
    state.contentEntries.unshift(entry)
    return json(entry, 201)
  }
  const match = path.match(
    /^\/api\/content\/pages\/([^/]+)(\/(publish|unpublish))?$/,
  )
  if (!match) return null
  const entry = state.contentEntries.find((item) => item.id === match[1])
  if (!entry) return json({ error: 'Content entry not found.' }, 404)
  if (method === 'PUT')
    Object.assign(entry, bodyOf(init), { updatedAt: DEMO_NOW })
  if (method === 'POST' && match[3]) {
    Object.assign(entry, {
      publishedAt: match[3] === 'publish' ? DEMO_NOW : null,
      status: match[3] === 'publish' ? 'published' : 'draft',
      updatedAt: DEMO_NOW,
    })
  }
  if (method === 'DELETE') {
    state.contentEntries = state.contentEntries.filter((item) => item !== entry)
  }
  return json(entry)
}

function handleSettings(path: string, method: string, init?: RequestInit) {
  if (!path.endsWith('/settings')) return null
  if (method === 'GET') return json(state.settings[path] ?? {})
  if (method === 'PUT') {
    state.settings[path] = bodyOf(init)
    return json(state.settings[path])
  }
  return null
}

function mockResponse(url: URL, method: string, init?: RequestInit): Response {
  const path = url.pathname.replace(DEMO_BASE_PATH, '')
  return (
    handleUsers(path, method, init) ??
    handleMemos(path, method, init) ??
    handleRedirects(path, method, init) ??
    handleForms(path, method, init) ??
    handleMedia(path, method, init, url) ??
    handleAudit(path, method, url) ??
    handleSearch(path, method, url) ??
    handleWebhooks(path, method, init, url) ??
    handleContent(path, method, init, url) ??
    handleSettings(path, method, init) ??
    json(
      { error: `No demo response is registered for ${method} ${path}.` },
      404,
    )
  )
}

function normalizeStaticRscResponse(url: URL, response: Response): Response {
  if (
    !response.ok ||
    !url.pathname.endsWith('.rsc') ||
    response.headers.get('Content-Type')?.startsWith('text/x-component')
  ) {
    return response
  }

  const headers = new Headers(response.headers)
  headers.set('Content-Type', 'text/x-component')

  // GitHub Pages serves unknown extensions as application/octet-stream.
  // Shadowing the immutable header collection preserves the fetched response's
  // URL and body, which vinext uses when deciding whether to navigate in-app.
  Object.defineProperty(response, 'headers', {
    configurable: true,
    value: headers,
  })
  return response
}

/** Installs the browser-only API simulator used by the public GitHub Pages demo. */
export function installMockApi(): void {
  if (installed || typeof window === 'undefined') return
  installed = true
  const nativeFetch = window.fetch.bind(window)
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const rawUrl = input instanceof Request ? input.url : String(input)
    const url = new URL(rawUrl, window.location.origin)
    const method = (
      init?.method ?? (input instanceof Request ? input.method : 'GET')
    ).toUpperCase()
    if (
      url.pathname === '/api' ||
      url.pathname.startsWith('/api/') ||
      url.pathname.startsWith(`${DEMO_BASE_PATH}/api/`)
    ) {
      return mockResponse(url, method, init)
    }
    return normalizeStaticRscResponse(url, await nativeFetch(input, init))
  }
}

/** Restores the deterministic seed data for another demo session. */
export function resetMockApi(): void {
  sequence = 100
  state = createDemoState()
}
