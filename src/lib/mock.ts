/**
 * 浏览器里跑 npm run dev 时的假后端（B 站数据）。
 * 只在「没有 Tauri 运行时」且「开发模式」下启用，打包后的应用不会用到。
 */
import type { Paged, RoomSnapshot, Track } from './types';

const cover = (seed: string) => `https://picsum.photos/seed/${encodeURIComponent(seed)}/336/336`;

function mk(id: string, title: string, artist: string, dur: number, plays: string): Track {
  return {
    source: 'bilibili',
    id,
    title,
    artist,
    cover: cover(title),
    duration_ms: dur,
    album: null,
    subtitle: plays,
  };
}

const BILI: Track[] = [
  mk('BV1GJ411x7h7', '【钢琴】夜曲 · 纯音乐版（一小时循环）', '琴键上的猫', 3_600_000, '82.3万播放'),
  mk('BV1xx411c7mD', '【VOCALOID】千年食谱颂 / 洛天依', 'ilem', 246_000, '356.1万播放'),
  mk('BV1Ks411p7Zq', '【翻唱】起风了 · 温柔女声版', '三无 MarBlue', 325_000, '91.2万播放'),
  mk('BV1i4411L7Kz', '【原创】雨夜电台 · lofi hiphop 合集', '深夜食堂音乐', 2_400_000, '14.5万播放'),
  mk('BV1YJ411q7xK', '【演奏】卡农 · 吉他指弹', '指弹小王子', 214_000, '68.9万播放'),
  mk('BV1uT4y1P7CX', '【国风】琵琶行 · 纯音乐', '墨韵随步摇', 291_000, '203.4万播放'),
  mk('BV1Jp4y1Y7bx', '【纯音乐】久石让组曲 · 一小时工作背景乐', '安静的书房', 3_600_000, '45.7万播放'),
  mk('BV1cs411E7Zu', '【民谣】成都 · 指弹版', '木吉他手阿杰', 268_000, '27.3万播放'),
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
    queue: BILI.slice(0, 4),
    state: { index: 0, position_ms: 0, playing: true, updated_at: Date.now(), by: 'me' },
    host_id: 'me',
    server_time: Date.now(),
  };
}

const handlers: Record<string, (args: Record<string, any>) => unknown> = {
  resolve_stream: ({ track }: any) => ({ stream_url: '', url: '', mime: 'audio/mp4', expires_at: Date.now() + 3_600_000, trackId: track?.id }),
  bili_search: ({ page }: any) => pageOf(BILI, page ?? 1),
  bili_ranking: ({ page }: any) => pageOf(BILI, page ?? 1),
  bili_related: () => BILI.slice(1),
  bili_detail: ({ bvid }: any) => ({ track: BILI.find((t) => t.id === bvid) ?? BILI[0], desc: '示例数据', pages: [[1, 'P1', 200]] }),
  bili_regions: () => [
    { name: '音乐区', id: 3 },
    { name: 'VOCALOID', id: 30 },
    { name: '演奏', id: 59 },
  ],
  bili_toview: () => BILI.slice(0, 3),
  bili_fav_folders: () => [{ id: 1, title: '我的收藏', media_count: 8 }],
  bili_fav_list: () => pageOf(BILI, 1),
  bili_me: () => ({ logged_in: false, name: '', avatar: '', mid: '' }),
  bili_qr: () => ({ key: 'mock', content: 'https://www.bilibili.com' }),
  bili_qr_poll: () => ({ code: 86101, message: '未扫描' }),
  bili_logout: () => null,

  get_settings: () => ({
    togetherServer: 'http://127.0.0.1:8787',
    nickname: '听友',
    bilibili: { name: '', avatar: '', loggedIn: false },
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

  together_cloud_host: () => ({ code: '123456', mode: 'cloud' }),
  together_cloud_join: () => ({ ok: true, mode: 'cloud' }),
  together_cloud_probe: ({ broker }: any) => ({ ok: true, broker: broker ?? 'wss://broker.emqx.io:8084/mqtt' }),
  together_am_host: () => true,
  together_ping: () => ({ ok: true }),
};

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const fn = handlers[cmd];
  if (!fn) throw new Error(`[mock] 未实现的命令: ${cmd}`);
  await new Promise((r) => setTimeout(r, 120));
  return fn(args ?? {}) as T;
}

export const MOCK_ENABLED: boolean =
  !!import.meta.env?.DEV && typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);
