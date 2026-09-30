import { create } from 'zustand';
import type { AuthUser } from '../lib/api/auth';

export type SessionStatus = 'loading' | 'authenticated' | 'anonymous';

interface AuthState {
  user: AuthUser | null;
  recorded: boolean;
  setSession: (user: AuthUser | null) => void;
  clear: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  recorded: false,
  setSession: (user) => set({ user, recorded: true }),
  clear: () => set({ user: null, recorded: true }),
}));
