import client from './client'

export interface LoginCredentials {
  login: string
  password: string
}

export interface TokenPair {
  access_token: string
  refresh_token: string
}
export interface SetPasswordPayload {
  token: string
  password: string
}
export async function login(credentials: LoginCredentials): Promise<TokenPair> {
  const response = await client.post<TokenPair>('/auth/login', credentials)
  return response.data
}

export async function refreshTokens(refreshToken: string): Promise<TokenPair> {
  const response = await client.post<TokenPair>('/auth/refresh', {
    refresh_token: refreshToken,
  })
  return response.data
}

export async function logout(refreshToken: string): Promise<void> {
  await client.post('/auth/logout', {
    refresh_token: refreshToken,
  })
}



export async function setPassword(payload: SetPasswordPayload): Promise<void> {
  await client.post('/auth/set-password', payload)
}