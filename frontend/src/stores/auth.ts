import { create } from 'zustand';
import type { User } from '../lib/types';

interface AuthState {
  user: User | null;
  token: string | null;
  isAuthenticated: boolean;
  login: (token: string, user: User) => void;
  logout: () => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  token: localStorage.getItem('menzi_token'),
  isAuthenticated: !!localStorage.getItem('menzi_token'),
  login: (token, user) => {
    localStorage.setItem('menzi_token', token);
    set({ token, user, isAuthenticated: true });
  },
  logout: () => {
    localStorage.removeItem('menzi_token');
    set({ token: null, user: null, isAuthenticated: false });
  },
}));
