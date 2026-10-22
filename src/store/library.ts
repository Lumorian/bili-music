import { create } from 'zustand';
import { api } from '../lib/api';
import type { Track } from '../lib/types';

const LS_KEY = 'bilimusic.library.v1';

interface PersistShape {
  favorites: Track[];
  recent: Track[];
}

export interface LibraryState extends PersistShape {
  setFavorites: (t: Track[]) => void;
  toggleFavorite: (t: Track) => void;
  isFavorite: (t: Track) => boolean;
  pushRecent: (t: Track) => void;
  clearRecent: () => void;
  loadFromDisk: () => Promise<void>;
}

function save(state: PersistShape) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify({ favorites: state.favorites, recent: state.recent }));
  } catch {
    /* 存储满了就算了 */
  }
}

function readDisk(): PersistShape {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return { favorites: [], recent: [] };
    const v = JSON.parse(raw);
    return {
      favorites: Array.isArray(v.favorites) ? v.favorites : [],
      recent: Array.isArray(v.recent) ? v.recent : [],
    };
  } catch {
    return { favorites: [], recent: [] };
  }
}

/** 本地曲库：收藏 + 最近播放（存在 WebView 的 localStorage 里） */
export const useLibrary = create<LibraryState>((set, get) => ({
  favorites: [],
  recent: [],

  setFavorites: (favorites) => {
    set({ favorites });
    save(get());
  },

  toggleFavorite: (t) => {
    const key = `${t.source}:${t.id}`;
    const exist = get().favorites.some((f) => `${f.source}:${f.id}` === key);
    const favorites = exist
      ? get().favorites.filter((f) => `${f.source}:${f.id}` !== key)
      : [t, ...get().favorites];
    set({ favorites });
    save(get());
  },

  isFavorite: (t) => get().favorites.some((f) => f.source === t.source && f.id === t.id),

  pushRecent: (t) => {
    const key = `${t.source}:${t.id}`;
    const recent = [t, ...get().recent.filter((r) => `${r.source}:${r.id}` !== key)].slice(0, 100);
    set({ recent });
    save(get());
  },

  clearRecent: () => {
    set({ recent: [] });
    save(get());
  },

  loadFromDisk: async () => {
    const disk = readDisk();
    set(disk);
    // 顺手探一下后端是否就绪；失败不影响本地收藏的展示
    try {
      await api.appInfo();
    } catch {
      /* 后端不可用时忽略 */
    }
  },
}));
