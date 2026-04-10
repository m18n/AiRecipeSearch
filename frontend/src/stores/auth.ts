import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { login as apiLogin, logout as apiLogout, refreshTokens as apiRefreshTokens } from '@/api/auth'
import type { LoginCredentials } from '@/api/auth'
import router from '@/router'

export const useAuthStore = defineStore('auth', () => {
  const accessToken = ref<string | null>(localStorage.getItem('accessToken'))
  const refreshToken = ref<string | null>(localStorage.getItem('refreshToken'))

  const isAuthenticated = computed(() => !!accessToken.value)

  function setTokens(newAccessToken: string, newRefreshToken: string) {
    accessToken.value = newAccessToken
    refreshToken.value = newRefreshToken
    localStorage.setItem('accessToken', newAccessToken)
    localStorage.setItem('refreshToken', newRefreshToken)
  }

  function clearTokens() {
    accessToken.value = null
    refreshToken.value = null
    localStorage.removeItem('accessToken')
    localStorage.removeItem('refreshToken')
  }

  async function login(credentials: LoginCredentials) {
    const data = await apiLogin(credentials)
    setTokens(data.access_token, data.refresh_token)
    await router.push('/search')
  }

  async function logout() {
    if (refreshToken.value) {
      try {
        await apiLogout(refreshToken.value)
      } catch {

      }
    }
    clearTokens()
    await router.push('/login')
  }

  async function refreshTokens() {
    if (!refreshToken.value) {
      throw new Error('No refresh token available')
    }
    const data = await apiRefreshTokens(refreshToken.value)
    setTokens(data.access_token, data.refresh_token)
  }

  return {
    accessToken,
    refreshToken,
    isAuthenticated,
    login,
    logout,
    refreshTokens,
    setTokens,
    clearTokens
  }
})