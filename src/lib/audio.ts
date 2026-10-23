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
    // 不要设 crossOrigin：我们只出声、不读样本，纯播放不需要 CORS。
    // 一旦设成 anonymous，媒体请求会走 CORS 模式，Chromium（Windows 的 WebView2）
    // 还会额外发预检、套 Private Network Access 规则，代理那边少一个头就整个播不出来。
    bind(el);
  }
  return el;
}

/** 偏差超过这个值就跟服务器对齐（毫秒） */
const DRIFT_TOLERANCE = 1200;

/** 本次进程里探测到的音频解码能力（null = 还没探测） */
let audioSupport: boolean | null = null;

/**
 * WebView 有没有可用的音频解码后端。
 *
 * 这个检查是保命用的：Linux 上 WebKitGTK 的 <audio> 依赖 GStreamer 插件，
 * 一旦插件缺失（AppImage 没打包、系统没装），**给 audio.src 赋值就会让
 * WebProcess 直接 abort**，表现出来就是「一播放整个窗口就没了」，还会伴随
 *   GLib-GObject-CRITICAL: invalid (NULL) pointer instance
 *   g_signal_connect_data: assertion 'G_TYPE_CHECK_INSTANCE (instance)' failed
 * 这类日志。canPlayType 在插件缺失时会返回空串，正好可以用来提前拦截。
 */
export function audioSupported(): boolean {
  if (audioSupport !== null) return audioSupport;
  audioSupport =
    audioCanPlay('audio/mp4; codecs="mp4a.40.2"') ||
    audioCanPlay('audio/mp4') ||
    audioCanPlay('audio/webm; codecs="opus"') ||
    audioCanPlay('audio/aac');
  return audioSupport;
}

/**
 * 探测用的 MIME。
 * 之所以同时问 mp4 和 webm：Android WebView 上 AAC（mp4a）支持很稳，
 * 但个别定制 ROM 的 WebView 会只报 webm/opus。两个都问一遍，
 * 免得在能播的机器上误判成"没有解码后端"而直接不给播。
 */
export function audioCanPlay(mime: string): boolean {
  try {
    return document.createElement('audio').canPlayType(mime) !== '';
  } catch {
    return false;
  }
}

/** 缺解码器时给用户一句能照着做的提示（只对 Linux 有意义） */
export const NO_AUDIO_BACKEND_MSG =
  '这台机器没有可用的音频解码后端。Linux（WebKitGTK 依赖 GStreamer）请安装：Debian/Ubuntu sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav；Arch sudo pacman -S gst-plugins-good gst-plugins-bad gst-libav。Windows / Android 出现这条说明系统 WebView 异常，建议重装 WebView2 / Android System WebView。';

/**
 * 自动播放被 WebView 拦下时，等下一次用户手势再试一次。
 * Windows 的 WebView2（Chromium）默认策略只允许"用户手势调用栈里"的 play()，
 * 而我们是先 invoke 后端解析地址、await 回来再 play()，手势上下文早没了。
 * 这里不把 playing 改成 false（否则界面和用户意图脱节、看起来永远停在暂停），
 * 而是在下一次点击/按键时补一次 play()。
 */
let gestureRetryArmed = false;

function armGestureRetry(a: HTMLAudioElement) {
  if (gestureRetryArmed) return;
  gestureRetryArmed = true;
  const retry = () => {
    gestureRetryArmed = false;
    window.removeEventListener('pointerdown', retry, true);
    window.removeEventListener('keydown', retry, true);
    if (usePlayer.getState().playing) tryPlay(a);
  };
  window.addEventListener('pointerdown', retry, true);
  window.addEventListener('keydown', retry, true);
}

/** 调 play()，并把三类失败分开对待（别把"换歌打断"当成"用户暂停"） */
function tryPlay(a: HTMLAudioElement) {
  const ret = a.play();
  if (!ret || typeof ret.catch !== 'function') return;
  ret.catch((err: DOMException) => {
    const name = err?.name ?? '';
    // AbortError：src 刚被换掉，这次 play() 必然被中断，等新地址自己来播
    if (name === 'AbortError') return;
    if (name === 'NotAllowedError') {
      armGestureRetry(a);
      return;
    }
    const p = usePlayer.getState();
    p.setPlaying(false);
    usePlayer.setState({ error: `播放失败：${err?.message || name || '未知错误'}` });
  });
}

/**
 * 每个（一次性）代理地址已经重试过几次。
 * B 站的播放地址带时效，放久了再点会 403；代理 token 是一次性的，
 * 所以"换一个地址重试"就等于换一个新的 token。
 */
const errorRetries = new Map<string, number>();

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

    // 一起听：新歌的元数据一到，立刻对齐到房间认为的位置。
    // 不这么做的话要等 App 里那个 2.5s 的心跳，切歌瞬间会和队友错开几百毫秒。
    const t = useTogether.getState();
    if (t.joined && t.connected && t.room && a.src === p.streamUrl) {
      const want = expectedPosition(t.room, t.clockOffset);
      if (Number.isFinite(want) && want > 0 && a.duration > 0) {
        a.currentTime = Math.min(want, a.duration * 1000) / 1000;
      }
    }
  });

  a.addEventListener('ended', () => {
    const t = useTogether.getState();
    if (t.joined && t.connected) {
      // 一起听模式下由房主推进，其他人等服务器广播
      if (t.isHost()) void import('./api').then(({ api }) => api.togetherNext().catch(() => {}));
      return;
    }
    const p = usePlayer.getState();
    // 单曲循环：直接回到开头重放，不重新解析音轨
    if (p.mode === 'single') {
      a.currentTime = 0;
      tryPlay(a);
      return;
    }
    p.next(true);
  });

  a.addEventListener('error', () => {
    const p = usePlayer.getState();
    if (!p.streamUrl) return;
    // 换歌时上一首的 error 可能晚一步才到，别拿它误伤正在播的这首
    if (a.src !== p.streamUrl) return;
    const code = a.error?.code ?? 0;
    const reason: Record<number, string> = {
      1: '播放被中止',
      2: '网络错误（音频代理连不上或上游拒绝）',
      3: '音频解码失败',
      4: '音频地址不可用（B 站的地址有时效，重新解析一次）',
    };

    // 网络/地址问题自动换一个地址重试一次：链接过期是最常见的偶发失败，
    // 让用户去手点一次"上一首下一首"很蠢。每个地址只给一次机会，避免死循环。
    const badUrl = p.streamUrl;
    const tried = errorRetries.get(badUrl) ?? 0;
    if ((code === 2 || code === 4) && tried < 1) {
      if (errorRetries.size > 200) errorRetries.clear();
      errorRetries.set(badUrl, tried + 1);
      void p.loadStream();
      return;
    }

    p.setPlaying(false);
    if (code) usePlayer.setState({ error: `${reason[code] ?? '播放失败'}（code ${code}）` });
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
  const p = usePlayer.getState();
  const t = useTogether.getState();

  // 没有解码后端时绝不能碰 audio 元素，否则 WebProcess 会崩
  if (!audioSupported()) {
    if (p.streamUrl && p.error !== NO_AUDIO_BACKEND_MSG) {
      usePlayer.setState({ error: NO_AUDIO_BACKEND_MSG, playing: false, streamUrl: null, loading: false });
    }
    return;
  }

  const a = getAudio();
  applyVolumes(a);

  // 没有可播地址（正在解析音轨，或者解析失败）：先把上一首停下来。
  // 否则新的一首还在解析，旧音频继续放，界面却已经显示成新歌。
  if (!p.streamUrl) {
    if (!a.paused) a.pause();
    return;
  }

  if (a.src !== p.streamUrl) {
    a.src = p.streamUrl;
    a.load();
    // 新地址只有加载完元数据才能 seek；把 store 里的 position 带上，
    // 否则"切歌 + 立刻拖进度条"会丢掉这一次拖动。
    if (p.position > 0) {
      const want = p.position / 1000;
      a.addEventListener(
        'loadedmetadata',
        () => {
          // 一起听模式下位置由房间决定（上面 loadedmetadata 里已经对齐过），
          // 这里再写一次会把房间的对齐结果盖掉
          const t = useTogether.getState();
          if (t.joined && t.connected) return;
          if (usePlayer.getState().streamUrl === p.streamUrl && a.readyState > 0) a.currentTime = want;
        },
        { once: true },
      );
    }
  }

  if (p.playing) {
    if (a.paused) tryPlay(a);
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
