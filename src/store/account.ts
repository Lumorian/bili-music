import { create } from 'zustand';
import { api } from '../lib/api';
import type { UserInfo } from '../lib/types';

/**
 * B 站账号状态。
 * 单独开一个 store，是因为侧边栏、设置页、我的音乐都要读它，
 * 而它们分散在不同页面里，靠 props 传会到处漏。
 */
interface AccountState {
  user: UserInfo | null;
  loading: boolean;
  refresh: () => Promise<UserInfo | null>;
  logout: () => Promise<void>;
  setUser: (u: UserInfo | null) => void;
}

export const useAccount = create<AccountState>((set) => ({
  user: null,
  loading: false,

  refresh: async () => {
    set({ loading: true });
    try {
      const me = await api.biliMe();
      set({ user: me, loading: false });
      return me;
    } catch {
      set({ loading: false });
      return null;
    }
  },

  logout: async () => {
    try {
      await api.biliLogout();
    } finally {
      set({ user: { logged_in: false, name: '', avatar: '', mid: '' } });
    }
  },

  setUser: (u) => set({ user: u }),
}));
