import { beforeEach, describe, expect, it } from 'vitest'
import { useAuthStore } from './auth'
import type { User } from '../lib/types'

const user: User = { id: 'u1', email: 'ada@example.com', name: 'Ada' }

beforeEach(() => {
  useAuthStore.setState({ user: null, token: null, isAuthenticated: false })
  localStorage.clear()
})

describe('auth store', () => {
  it('starts unauthenticated', () => {
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
    expect(useAuthStore.getState().token).toBeNull()
  })

  it('logs in and persists the token', () => {
    useAuthStore.getState().login('tok-1', user)
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(true)
    expect(state.token).toBe('tok-1')
    expect(state.user).toEqual(user)
    expect(localStorage.getItem('menzi_token')).toBe('tok-1')
  })

  it('logs out and clears the session', () => {
    useAuthStore.getState().login('tok-1', user)
    useAuthStore.getState().logout()
    const state = useAuthStore.getState()
    expect(state.isAuthenticated).toBe(false)
    expect(state.token).toBeNull()
    expect(state.user).toBeNull()
    expect(localStorage.getItem('menzi_token')).toBeNull()
  })
})