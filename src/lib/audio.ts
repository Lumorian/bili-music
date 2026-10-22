import { usePlayer } from '../store/player';
import { useTogether, expectedPosition } from '../store/together';
import { useLibrary } from '../store/library';

/**
 * 全局唯一 audio 元素 + 播放逻辑。
 * 单独抽出来是因为：
 * - 一起听模式要"以服务器时间为准"纠正偏差，而不是本地随意播；
 * - 切歌由 store 驱动，DOM 只负责老老实实出声。
 */
let el: HTMLAudioElement | null = null;
let lastSyncSend = 0;
let seekGuardUntil = 0;

export function getAudio(): HTMLAudioElement {
  if (!el) {
    el = new Audio();
    el.preload = 'auto';
    el.crossOrigin = 'anonymous';
    bind(el);
  }
  return el;
}

/** 偏差超过这个值就跟服务器对齐（毫秒） */
const DRIFT_TOLERANCE = 1200;

function bind(a: HTMLAudioElement) {
  a.addEventListener('timeupdate', () => {
    const p = usePlayer.getState();
    p.tick(a.currentTime * 1000, (a.duration || 0) * 1000);

    // 一起听：定时上报自己的进度，给队友的画小进度条
    const t = useTogether.getState();
    if (t.joined && t.connected && Date.now() - lastSyncSend > 4000) {
      lastSyncSend = Date.now();
      void import('./api').then(({ api }) =>
        api.togetherProgress(Math.round(a.currentTime * 1000), !a.paused).catch(() => {}),
      );
    }
  });

  a.addEventListener('loadedmetadata', () => {
    const p = usePlayer.getState();
    p.tick(a.currentTime * 1000, a.duration * 1000);
  });

  a.addEventListener('ended', () => {
    const t = useTogether.getState();
    if (t.joined && t.connected) {
      // 一起听模式下由房主推进，其他人等服务器广播
      if (t.isHost()) void import('./api').then(({ api }) => api.togetherNext().catch(() => {}));
      return;
    }
    const p = usePlayer.getState();
    p.next(true);
  });

  a.addEventListener('error', () => {
    const p = usePlayer.getState();
    if (!p.streamUrl) return;
    p.setPlaying(false);
  });

  a.addEventListener('play', () => {
    const track = usePlayer.getState().queue[usePlayer.getState().index];
    if (track) useLibrary.getState().pushRecent(track);
  });
}

/** 把 store 里的 streamUrl / playing / volume 同步到 audio 元素 */
export function applyVolumes(a: HTMLAudioElement) {
  const p = usePlayer.getState();
  a.volume = p.muted ? 0 : p.volume;
}

export function syncFromState() {
  const a = getAudio();
  const p = usePlayer.getState();
  const t = useTogether.getState();

  applyVolumes(a);

  if (p.streamUrl && a.src !== p.streamUrl) {
    a.src = p.streamUrl;
    a.load();
  }
  // 浏览器 mock 环境没有真实音频，别去 play() 制造报错
  if (!p.streamUrl) return;

  if (p.playing) {
    if (a.paused) void a.play().catch(() => p.setPlaying(false));
  } else if (!a.paused) {
    a.pause();
  }

  // 一起听：把播放位置拉到服务器认为的位置
  if (t.joined && t.connected && t.room) {
    if (Date.now() < seekGuardUntil) return;
    const want = expectedPosition(t.room, t.clockOffset);
    const have = a.currentTime * 1000;
    if (Number.isFinite(want) && Math.abs(want - have) > DRIFT_TOLERANCE && a.readyState > 0) {
      a.currentTime = want / 1000;
    }
  }
}

/** 用户手动拖动时先别纠正，等服务器回声回来 */
export function guardSeek(ms = 2500) {
  seekGuardUntil = Date.now() + ms;
}
