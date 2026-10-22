/**
 * 浏览器里跑 npm run dev 时的假后端。
 * 只在「没有 Tauri 运行时」且「开发模式」下启用，打包后的应用不会用到。
 * 目的：调 UI 时不用每次都启动整个 Tauri 进程。
 */
import type { Paged, RoomSnapshot, Track } from './types';

const cover = (seed: string) => `https://picsum.photos/seed/${encodeURIComponent(seed)}/336/336`;

function mk(source: 'bilibili' | 'netease', id: string, title: string, artist: string, dur: number): Track {
  return {
    source,
    id,
    title,
    artist,
    cover: cover(title),
    duration_ms: dur,
    album: source === 'netease' ? '示例专辑' : null,
    subtitle: source === 'bilibili' ? '12.3万播放' : null,
  };
}

const BILI: Track[] = [
  mk('bilibili', 'BV1GJ411x7h7', '【钢琴】夜曲 · 纯音乐版（一小时循环）', '琴键上的猫', 3_600_000),
  mk('bilibili', 'BV1xx411c7mD', '【VOCALOID】千年食谱颂 / 洛天依', 'ilem', 246_000),
  mk('bilibili', 'BV1Ks411p7Zq', '【翻唱】起风了 · 温柔女声版', '三无 MarBlue', 325_000),
  mk('bilibili', 'BV1i4411L7Kz', '【原创】雨夜电台 · lofi hiphop 合集', '深夜食堂音乐', 2_400_000),
  mk('bilibili', 'BV1YJ411q7xK', '【演奏】卡农 · 吉他指弹', '指弹小王子', 214_000),
  mk('bilibili', 'BV1uT4y1P7CX', '【国风】琵琶行 · 纯音乐', '墨韵随步摇', 291_000),
];

const NE: Track[] = [
  mk('netease', '186016', '晴天', '周杰伦', 269_000),
  mk('netease', '5257138', '屋顶', '周杰伦 / 温岚', 320_000),
  mk('netease', '385781', '突然好想你', '五月天', 355_000),
  mk('netease', '1330348068', '起风了', '买辣椒也用券', 325_000),
  mk('netease', '1901371647', '海底', '一支榴莲', 260_000),
  mk('netease', '347230', '海阔天空', 'Beyond', 326_000),
];

function pageOf(items: Track[], page = 1, size = 20): Paged<Track> {
  const start = (page - 1) * size;
  return { items: items.slice(start, start + size), page, has_more: start + size < items.length, total: items.length };
}

function mockRoom(): RoomSnapshot {
  return {
    code: '123456',
    name: '示例房间',
    members: [
      { id: 'me', name: '我', avatar: '', is_host: true, position_ms: 0, playing: true },
      { id: 'friend', name: '好友', avatar: '', is_host: false, position_ms: 0, playing: true },
    ],
    queue: NE.slice(0, 4),
    state: { index: 0, position_ms: 0, playing: true, updated_at: Date.now(), by: 'me' },
    host_id: 'me',
    server_time: Date.now(),
  };
}

const handlers: Record<string, (args: Record<string, any>) => unknown> = {
  resolve_stream: ({ track }: any) => ({ stream_url: '', url: '', mime: 'audio/mpeg', expires_at: Date.now() + 3_600_000, trackId: track?.id }),
  bili_search: ({ page }: any) => pageOf(BILI, page ?? 1),
  bili_ranking: ({ page }: any) => pageOf(BILI, page ?? 1),
  bili_related: () => BILI.slice(1),
  bili_detail: ({ bvid }: any) => ({ track: BILI.find((t) => t.id === bvid) ?? BILI[0], desc: '示例数据', pages: [[1, 'P1', 200]] }),
  bili_regions: () => [],
  bili_toview: () => BILI.slice(0, 3),
  bili_fav_folders: () => [{ id: 1, title: '我的收藏', media_count: 6 }],
  bili_fav_list: () => pageOf(BILI, 1),
  bili_me: () => ({ logged_in: false, name: '', avatar: '', mid: '' }),
  bili_qr: () => ({ key: 'mock', content: 'https://www.bilibili.com' }),
  bili_qr_poll: () => ({ code: 86101, message: '未扫描' }),
  bili_logout: () => null,

  ne_search: ({ page }: any) => pageOf(NE, page ?? 1),
  ne_song: ({ id }: any) => NE.find((t) => t.id === id) ?? NE[0],
  ne_lyric: () => ({ lrc: '[00:00.00] 示例歌词\n[00:04.00] 这是一行歌词\n[00:08.00] 浏览器调试模式下没有真实歌词\n[00:12.00] 打开 Tauri 才能连真实接口', translation: '' }),
  ne_toplists: () => [
    { name: '飙升榜', id: '19723756' },
    { name: '新歌榜', id: '3779629' },
    { name: '热歌榜', id: '3778678' },
  ],
  ne_playlist: () => ({ name: '示例榜单', tracks: NE }),
  ne_my_playlists: () => [],
  ne_me: () => ({ logged_in: false, name: '', avatar: '', mid: '' }),
  ne_qr: () => ({ key: 'mock', content: 'https://music.163.com' }),
  ne_qr_poll: () => ({ code: 801, message: '等待扫码' }),
  ne_logout: () => null,

  get_settings: () => ({
    togetherServer: 'http://127.0.0.1:8787',
    nickname: '听友',
    bilibili: { name: '', avatar: '', loggedIn: false },
    netease: { name: '', avatar: '', loggedIn: false },
  }),
  save_settings: () => null,
  app_info: () => ({ version: '0.1.0-mock', proxyPort: 0, platform: 'browser' }),

  together_create: () => ({ code: '123456', room: mockRoom() }),
  together_join: () => ({ ok: true }),
  together_snapshot: () => ({ selfId: 'me', room: null }),
  together_leave: () => null,
  together_chat: () => null,
  together_set_queue: () => null,
  together_add_track: () => null,
  together_remove_track: () => null,
  together_play_index: () => null,
  together_set_playing: () => null,
  together_seek: () => null,
  together_next: () => null,
  together_prev: () => null,
  together_progress: () => null,
  together_transfer_host: () => null,
  together_kick: () => null,
  together_default_server: () => 'http://127.0.0.1:8787',
  together_ping: () => ({ ok: true }),
};

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const fn = handlers[cmd];
  if (!fn) throw new Error(`[mock] 未实现的命令: ${cmd}`);
  // 模拟一点网络延迟，方便观察 loading 态
  await new Promise((r) => setTimeout(r, 120));
  return fn(args ?? {}) as T;
}

export const MOCK_ENABLED: boolean =
  !!import.meta.env?.DEV && typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);
