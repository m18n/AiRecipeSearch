import axios from 'axios'
import type { AxiosInstance, InternalAxiosRequestConfig } from 'axios'
import router from '@/router'
import { RateLimitError } from '@/types/rateLimit'

const client: AxiosInstance = axios.create({
  baseURL: '/api/v1',
  headers: { 'Content-Type': 'application/json' },
})


client.interceptors.request.use(
  (config: InternalAxiosRequestConfig) => {
    const accessToken = localStorage.getItem('accessToken')
    if (accessToken) config.headers.Authorization = `Bearer ${accessToken}`
    return config
  },
  (error) => Promise.reject(error),
)


client.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config


    if (error.response?.status === 429) {
      const data = error.response.data
      return Promise.reject(
        new RateLimitError(
          data?.message ?? 'Too many requests. Please try again later.',
          data?.retry_after_minutes ?? 5,
        ),
      )
    }


    if (error.response?.status === 401 && !originalRequest._retry) {
      originalRequest._retry = true
      const refreshToken = localStorage.getItem('refreshToken')

      if (!refreshToken) {
        await handleAuthFailure()
        return Promise.reject(error)
      }

      try {
        const { data } = await axios.post('/api/v1/auth/refresh', {
          refresh_token: refreshToken,
        })
        const { useAuthStore } = await import('@/stores/auth')
        useAuthStore().setTokens(data.access_token, data.refresh_token)
        originalRequest.headers.Authorization = `Bearer ${data.access_token}`
        return client(originalRequest)
      } catch {
        await handleAuthFailure()
        return Promise.reject(error)
      }
    }


    if (error.response?.status === 422) {
      return Promise.reject(error.response.data)
    }

    return Promise.reject(error)
  },
)

async function handleAuthFailure(): Promise<void> {
  const { useAuthStore } = await import('@/stores/auth')
  useAuthStore().clearTokens()
  await router.push('/login')
}

export default client