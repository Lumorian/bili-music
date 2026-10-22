import { create } from 'zustand';
import { api } from '../lib/api';
import type { ChatItem, MemberInfo, RoomSnapshot } from '../lib/types';

export interface TogetherState {
  joined: boolean;
  connecting: boolean;
  connected: boolean;
  room: RoomSnapshot | null;
  selfId: string;
  server: string;
  code: string;
  nickname: string;
  chats: ChatItem[];
  error: string | null;
  /** 服务器与本地时钟差（毫秒），用于对齐播放进度 */
  clockOffset: number;

  init: () => Promise<void>;
  createRoom: (server: string, name: string) => Promise<string>;
  joinRoom: (server: string, code: string, name: string, avatar: string) => Promise<void>;
  leave: () => Promise<void>;
  sendChat: (text: string) => Promise<void>;

  /** 事件入口，由 Tauri 事件监听调用 */
  onEvent: (ev: any) => void;
  isHost: () => boolean;
  self: () => MemberInfo | null;
}

export const useTogether = create<TogetherState>((set, get) => ({
  joined: false,
  connecting: false,
  connected: false,
  room: null,
  selfId: '',
  server: 'http://127.0.0.1:8787',
  code: '',
  nickname: '',
  chats: [],
  error: null,
  clockOffset: 0,

  init: async () => {
    try {
      const server = await api.togetherDefaultServer();
      const settings = await api.getSettings();
      set({ server, nickname: settings.nickname });
    } catch {
      /* 忽略：没启动后端时静默 */
    }
  },

  createRoom: async (server, name) => {
    set({ connecting: true, error: null });
    try {
      const { code } = await api.togetherCreate(server, name);
      set({ connecting: false, server, code });
      return code;
    } catch (e) {
      set({ connecting: false, error: (e as Error).message });
      throw e;
    }
  },

  joinRoom: async (server, code, name, avatar) => {
    set({ connecting: true, error: null });
    try {
      await api.togetherJoin({ server, code, name, avatar });
      set({ joined: true, connecting: false, server, code, nickname: name, chats: [] });
    } catch (e) {
      set({ connecting: false, error: (e as Error).message });
      throw e;
    }
  },

  leave: async () => {
    try {
      await api.togetherLeave();
    } finally {
      set({ joined: false, connected: false, room: null, selfId: '', code: '', chats: [] });
    }
  },

  sendChat: async (text) => {
    const clean = text.trim();
    if (!clean) return;
    await api.togetherChat(clean);
  },

  onEvent: (ev) => {
    const kind = ev?.kind;
    if (kind === 'connected') {
      const offset = (ev.room?.server_time ?? Date.now()) - Date.now();
      set({ connected: true, selfId: ev.selfId, room: ev.room, clockOffset: offset, joined: true });
      return;
    }
    if (kind === 'room') {
      const prev = get().room;
      const room: RoomSnapshot = ev.room;
      // 保留本地聊天记录，房间快照不含它
      set({
        room,
        clockOffset: room.server_time - Date.now(),
        // 自己不在成员列表里说明被踢了
        joined: prev ? room.members.some((m) => m.id === get().selfId) : true,
      });
      return;
    }
    if (kind === 'state') {
      const room = get().room;
      if (!room) return;
      set({
        room: {
          ...room,
          state: {
            index: ev.index,
            position_ms: ev.positionMs,
            playing: ev.playing,
            updated_at: ev.serverTime,
            by: ev.by,
          },
          server_time: ev.serverTime,
        },
        clockOffset: ev.serverTime - Date.now(),
      });
      return;
    }
    if (kind === 'queue') {
      const room = get().room;
      if (!room) return;
      set({ room: { ...room, queue: ev.queue, state: { ...room.state, index: ev.index } } });
      return;
    }
    if (kind === 'chat') {
      const item: ChatItem = { from: ev.from, name: ev.name, text: ev.text, at: ev.at };
      set({ chats: [...get().chats, item].slice(-200) });
      return;
    }
    if (kind === 'disconnected') {
      set({ connected: false, joined: false, room: null, error: ev.reason ?? '已断开' });
      return;
    }
    if (kind === 'error') {
      set({ error: ev.message });
    }
  },

  isHost: () => {
    const { room, selfId } = get();
    return !!room && room.host_id === selfId;
  },

  self: () => {
    const { room, selfId } = get();
    return room?.members.find((m) => m.id === selfId) ?? null;
  },
}));

/** 把房间状态换算成"此刻应该播放到哪里" */
export function expectedPosition(room: RoomSnapshot, clockOffset: number): number {
  const base = room.state.position_ms;
  if (!room.state.playing) return base;
  const now = Date.now() + clockOffset;
  const elapsed = Math.max(0, now - room.state.updated_at);
  const total = room.queue[room.state.index]?.duration_ms ?? 0;
  const pos = base + elapsed;
  return total > 0 ? Math.min(pos, total) : pos;
}
