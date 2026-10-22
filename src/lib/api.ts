import { invoke } from '@tauri-apps/api/core';
import type {
  Paged,
  QrPayload,
  RoomSnapshot,
  Settings,
  Source,
  StreamInfo,
  Track,
  TrackDetail,
  UserInfo,
} from './types';

import { MOCK_ENABLED, mockInvoke } from './mock';

/** 所有后端命令的统一出口，出错了转成可读中文 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    // 直接在浏览器里打开 dev server 时没有 Tauri 运行时，用假数据把界面跑起来
    if (MOCK_ENABLED) return await mockInvoke<T>(cmd, args);
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (typeof e === 'string') throw new Error(e);
    throw new Error((e as Error)?.message ?? '未知错误');
  }
}

export const api = {
  // ---- 播放 ----
  resolveStream: (track: Track) => call<StreamInfo>('resolve_stream', { track }),

  // ---- B 站 ----
  biliSearch: (keyword: string, page = 1) => call<Paged<Track>>('bili_search', { keyword, page }),
  biliRanking: (rid = 3, page = 1) => call<Paged<Track>>('bili_ranking', { rid, page }),
  biliRelated: (bvid: string) => call<Track[]>('bili_related', { bvid }),
  biliDetail: (bvid: string) => call<TrackDetail>('bili_detail', { bvid }),
  biliRegions: () => call<{ name: string; id: number }[]>('bili_regions'),
  biliToview: () => call<Track[]>('bili_toview'),
  biliFavFolders: () => call<{ id: number; title: string; media_count: number }[]>('bili_fav_folders'),
  biliFavList: (mediaId: number, page = 1) =>
    call<Paged<Track>>('bili_fav_list', { mediaId, page }),
  biliMe: () => call<UserInfo>('bili_me'),
  biliQr: () => call<QrPayload>('bili_qr'),
  biliQrPoll: (key: string) => call<{ code: number; message: string }>('bili_qr_poll', { key }),
  biliLogout: () => call<void>('bili_logout'),

  // ---- 网易云 ----
  neSearch: (keyword: string, page = 1) => call<Paged<Track>>('ne_search', { keyword, page }),
  neSong: (id: string) => call<Track>('ne_song', { id }),
  neLyric: (id: string) => call<{ lrc: string; translation: string }>('ne_lyric', { id }),
  neToplists: () => call<{ name: string; id: string }[]>('ne_toplists'),
  nePlaylist: (id: string) => call<{ name: string; tracks: Track[] }>('ne_playlist', { id }),
  neMyPlaylists: () => call<{ name: string; id: string; count: number }[]>('ne_my_playlists'),
  neMe: () => call<UserInfo>('ne_me'),
  neQr: () => call<QrPayload>('ne_qr'),
  neQrPoll: (key: string) => call<{ code: number; message: string }>('ne_qr_poll', { key }),
  neLogout: () => call<void>('ne_logout'),

  // ---- 设置 ----
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (togetherServer?: string, nickname?: string) =>
    call<void>('save_settings', { togetherServer, nickname }),
  appInfo: () => call<{ version: string; proxyPort: number; platform: string }>('app_info'),

  // ---- 一起听 ----
  togetherCreate: (server: string, name: string) =>
    call<{ code: string; room: RoomSnapshot }>('together_create', { server, name }),
  togetherJoin: (args: { server: string; code: string; name: string; avatar: string }) =>
    call<{ ok: boolean }>('together_join', { args }),
  togetherSnapshot: () =>
    call<{ selfId: string; room: RoomSnapshot | null }>('together_snapshot'),
  togetherLeave: () => call<void>('together_leave'),
  togetherChat: (text: string) => call<void>('together_chat', { text }),
  togetherSetQueue: (queue: Track[], index: number) =>
    call<void>('together_set_queue', { queue, index }),
  togetherAddTrack: (track: Track, next = false) =>
    call<void>('together_add_track', { track, next }),
  togetherRemoveTrack: (index: number) =>
    call<void>('together_remove_track', { index }),
  togetherPlayIndex: (index: number, positionMs: number) =>
    call<void>('together_play_index', { index, positionMs }),
  togetherSetPlaying: (playing: boolean, positionMs: number) =>
    call<void>('together_set_playing', { playing, positionMs }),
  togetherSeek: (positionMs: number) => call<void>('together_seek', { positionMs }),
  togetherNext: () => call<void>('together_next'),
  togetherPrev: () => call<void>('together_prev'),
  togetherProgress: (positionMs: number, playing: boolean) =>
    call<void>('together_progress', { positionMs, playing }),
  togetherTransferHost: (memberId: string) =>
    call<void>('together_transfer_host', { memberId }),
  togetherKick: (memberId: string) => call<void>('together_kick', { memberId }),
  togetherDefaultServer: () => call<string>('together_default_server'),
  togetherPing: (server: string) => call<{ ok: boolean }>('together_ping', { server }),
};

export type { Source };
