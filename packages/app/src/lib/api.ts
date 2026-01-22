import { createApi } from '@devup-api/fetch'

export const api = createApi(process.env.API_URL || 'http://localhost:3001')
