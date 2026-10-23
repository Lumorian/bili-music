import { create } from 'zustand';
import { api } from '../lib/api';
import { trackKey } from '../lib/format';
import type { PlayMode, Track } from '../lib/types';

export interface PlayerState {
  queue: Track[];
  index: number;
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  muted: boolean;
  mode: PlayMode;
  loading: boolean;
  error: string | null;
  streamUrl: string | null;

  /** 一起听模式下，播放控制要交给房间服务器 */
  syncMode: boolean;

  history: Track[];

  setQueue: (queue: Track[], index?: number, autoplay?: boolean) => void;
  playTrack: (track: Track) => void;
  playAt: (index: number, positionMs?: number) => void;
  enqueue: (track: Track, next?: boolean) => void;
  removeAt: (index: number) => void;
  clearQueue: () => void;

  toggle: () => void;
  setPlaying: (playing: boolean) => void;
  next: (auto?: boolean) => void;
  prev: () => void;
  seek: (ms: number) => void;
  setVolume: (v: number) => void;
  toggleMute: () => void;
  cycleMode: () => void;

  /** 由 audio 元素回调 */
  tick: (position: number, duration: number) => void;
  loadStream: () => Promise<void>;
}

/**
 * 音轨解析的请求序号。
 *
 * resolve_stream 是异步的（Windows 上往往还要先查一次 cid 才能拿到音轨地址），
 * 用户在这期间完全可能已经点到下一首。旧请求晚回来时如果照样写进 state，
 * 就会出现「界面显示 B、耳朵里放着 A」，而且紧接着的那次 play() 会被
 * 换 src 打断，表现成"莫名暂停、怎么切都没用"。所以只认最后一次。
 */
let loadSeq = 0;

function shuffledNext(length: number, current: number): number {
  if (length <= 1) return 0;
  let n = current;
  while (n === current) n = Math.floor(Math.random() * length);
  return n;
}

export const usePlayer = create<PlayerState>((set, get) => ({
  queue: [],
  index: -1,
  playing: false,
  position: 0,
  duration: 0,
  volume: 0.85,
  muted: false,
  mode: 'order',
  loading: false,
  error: null,
  streamUrl: null,
  syncMode: false,
  history: [],

  setQueue: (queue, index = 0, autoplay = true) => {
    set({ queue, index: Math.max(0, Math.min(index, queue.length - 1)) });
    if (autoplay) {
      set({ playing: true });
      void get().loadStream();
    }
  },

  playTrack: (track) => {
    const { queue } = get();
    const key = trackKey(track);
    const exist = queue.findIndex((t) => trackKey(t) === key);
    if (exist >= 0) {
      get().playAt(exist);
      return;
    }
    const next = [...queue];
    next.splice(get().index + 1, 0, track);
    set({ queue: next, index: get().index + 1, playing: true, position: 0 });
    void get().loadStream();
  },

  playAt: (index, positionMs = 0) => {
    const { queue } = get();
    if (index < 0 || index >= queue.length) return;
    set({ index, position: positionMs, playing: true, error: null });
    void get().loadStream();
  },

  enqueue: (track, next = false) => {
    const { queue, index } = get();
    const copy = [...queue];
    if (next) copy.splice(index + 1, 0, track);
    else copy.push(track);
    set({ queue: copy });
    if (get().index < 0) set({ index: 0 });
  },

  removeAt: (index) => {
    const { queue, index: cur } = get();
    if (index < 0 || index >= queue.length) return;
    const copy = queue.filter((_, i) => i !== index);
    let nextIndex = cur;
    const removedCurrent = index === cur;
    if (index < cur) nextIndex = cur - 1;
    else if (removedCurrent) nextIndex = Math.min(cur, copy.length - 1);
    set({ queue: copy, index: nextIndex });
    // 删掉的是正在播的那首：必须重新拉流，否则界面换成了下一首、
    // 耳朵里还在放被删掉的那首（和切歌竞态是同一类症状）。
    if (removedCurrent) {
      set({ position: 0, streamUrl: null });
      if (copy.length) void get().loadStream();
      else set({ playing: false });
    }
  },

  clearQueue: () => set({ queue: [], index: -1, playing: false, streamUrl: null, position: 0 }),

  toggle: () => {
    const { playing } = get();
    if (!get().streamUrl && !playing) {
      void get().loadStream();
    }
    set({ playing: !playing });
  },

  setPlaying: (playing) => set({ playing }),

  next: (auto = false) => {
    const { queue, index, mode } = get();
    if (!queue.length) return;
    // 单曲循环：音频元素自己回到开头就行，不用重新解析音轨
    // （原来每次都重拉，等于每循环一遍都要走一次网络 + 换一个代理 token）
    if (mode === 'single' && auto) return;

    const atEnd = index + 1 >= queue.length;
    // 顺序播放：自动播到最后一首就停住，别偷偷绕回第一首
    if (auto && mode === 'order' && atEnd) {
      // 停在末尾：保留 position（进度条停在终点），只把播放状态关掉
      set({ playing: false });
      return;
    }
    let n: number;
    if (mode === 'shuffle') n = shuffledNext(queue.length, index);
    else n = atEnd ? 0 : index + 1;
    set({ index: n, position: 0, playing: true });
    void get().loadStream();
  },

  prev: () => {
    const { queue, index, mode } = get();
    if (!queue.length) return;
    let n: number;
    if (mode === 'shuffle') n = shuffledNext(queue.length, index);
    else n = index - 1 < 0 ? queue.length - 1 : index - 1;
    set({ index: n, position: 0, playing: true });
    void get().loadStream();
  },

  seek: (ms) => set({ position: ms }),

  setVolume: (v) => set({ volume: Math.min(1, Math.max(0, v)), muted: v === 0 }),

  toggleMute: () => set({ muted: !get().muted }),

  cycleMode: () => {
    const order: PlayMode[] = ['order', 'repeat', 'single', 'shuffle'];
    const cur = order.indexOf(get().mode);
    set({ mode: order[(cur + 1) % order.length] });
  },

  tick: (position, duration) => set({ position, duration: duration || get().duration }),

  loadStream: async () => {
    const { queue, index } = get();
    const track = queue[index];
    if (!track) return;
    const seq = ++loadSeq;
    // 先把上一首的地址清掉：解析期间要是还留着旧 URL，旧音频会继续出声，
    // 而界面已经换成了新的一首（就是"显示 B、播 A"）。
    set({ loading: true, error: null, streamUrl: null });
    try {
      const info = await api.resolveStream(track);
      // 解析期间用户切歌 / 换队列了，这份结果已经过期，丢掉
      if (seq !== loadSeq) return;
      const history = [track, ...get().history.filter((t) => trackKey(t) !== trackKey(track))].slice(0, 100);
      set({ streamUrl: info.stream_url, loading: false, duration: track.duration_ms, history });
    } catch (e) {
      if (seq !== loadSeq) return;
      set({ loading: false, error: (e as Error).message, playing: false, streamUrl: null });
    }
  },
}));

export const currentTrack = (s: PlayerState): Track | null =>
  s.index >= 0 && s.index < s.queue.length ? s.queue[s.index] : null;
