import { useCallback, useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Sidebar } from './components/Sidebar';
import { Player } from './components/Player';
import { NowPlaying } from './components/NowPlaying';
import { QueuePanel } from './components/QueuePanel';
import { QrLogin } from './components/QrLogin';
import { ToastHost, toast } from './components/Toast';
import { Home } from './pages/Home';
import { Search } from './pages/Search';
import { Discover } from './pages/Discover';
import { Library } from './pages/Library';
import { Together } from './pages/Together';
import { Settings } from './pages/Settings';
import { usePlayer } from './store/player';
import { useTogether } from './store/together';
import { useLibrary } from './store/library';
import { syncFromState } from './lib/audio';
import { api } from './lib/api';
import { MOCK_ENABLED } from './lib/mock';
import { IconHome, IconLibrary, IconSearch, IconUsers } from './components/Icons';

import type { PageId } from './lib/types';

export function App() {
  const [page, setPage] = useState<PageId>('home');
  const [queueOpen, setQueueOpen] = useState(false);
  const [nowOpen, setNowOpen] = useState(false);
  const [login, setLogin] = useState<'bilibili' | 'netease' | null>(null);
  const [version, setVersion] = useState('0.1.0');
  const [narrow, setNarrow] = useState(window.innerWidth <= 900);

  const player = usePlayer();
  const error = usePlayer((s) => s.error);
  const loadFromDisk = useLibrary((s) => s.loadFromDisk);
  const onTogetherEvent = useTogether((s) => s.onEvent);
  const initTogether = useTogether((s) => s.init);

  // 首屏初始化
  useEffect(() => {
    void loadFromDisk();
    void initTogether();
    api
      .appInfo()
      .then((i) => setVersion(i.version))
      .catch(() => {});
  }, [loadFromDisk, initTogether]);

  // 一起听事件（浏览器 mock 环境没有 Tauri 事件总线，跳过）
  useEffect(() => {
    if (MOCK_ENABLED) return;
    let unlisten: (() => void) | undefined;
    listen('together://event', (ev) => onTogetherEvent(ev.payload)).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [onTogetherEvent]);

  // 播放状态 → audio 元素
  const streamUrl = player.streamUrl;
  const playing = player.playing;
  const volume = player.volume;
  const muted = player.muted;
  useEffect(() => {
    syncFromState();
  }, [streamUrl, playing, volume, muted]);

  // 播放错误提示
  const lastError = useRef('');
  useEffect(() => {
    if (error && error !== lastError.current) {
      lastError.current = error;
      toast(error, 'error');
    }
  }, [error]);

  // 一起听：房间状态 → 本地播放器。
  // 判重逻辑：只有"当前这首真的换了"才重新拉流，否则加歌/聊天都会打断正在播的歌。
  useEffect(() => {
    let lastQueueSig = '';
    let lastTrackKey = '';
    let lastPlaying: boolean | null = null;

    const unsub = useTogether.subscribe((state) => {
      const room = state.room;
      if (!state.joined || !room) {
        lastQueueSig = '';
        lastTrackKey = '';
        lastPlaying = null;
        return;
      }
      const p = usePlayer.getState();
      const queueSig = room.queue.map((t) => `${t.source}:${t.id}`).join('|');
      if (queueSig !== lastQueueSig) {
        lastQueueSig = queueSig;
        p.setQueue(room.queue, room.state.index, false);
      }
      const cur = room.queue[room.state.index];
      const key = cur ? `${cur.source}:${cur.id}` : '';
      if (key && key !== lastTrackKey) {
        lastTrackKey = key;
        p.playAt(room.state.index, room.state.position_ms);
      }
      if (room.state.playing !== lastPlaying) {
        lastPlaying = room.state.playing;
        p.setPlaying(room.state.playing);
      }
    });
    return () => unsub();
  }, []);

  // 一起听时定期把本地进度往服务器时间上拉，避免慢慢跑偏
  useEffect(() => {
    const id = window.setInterval(() => {
      const t = useTogether.getState();
      if (t.joined && t.connected && t.room) syncFromState();
    }, 2500);
    return () => window.clearInterval(id);
  }, []);

  useEffect(() => {
    const onResize = () => setNarrow(window.innerWidth <= 900);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  // 空格键播放/暂停（输入框里不生效）
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;
      if (e.code === 'Space') {
        e.preventDefault();
        usePlayer.getState().toggle();
      } else if (e.code === 'ArrowRight' && e.shiftKey) {
        usePlayer.getState().next();
      } else if (e.code === 'ArrowLeft' && e.shiftKey) {
        usePlayer.getState().prev();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const onLoginSuccess = useCallback(() => {
    setLogin(null);
    // 重新挂载当前页面以刷新登录态
    setPage((p: PageId) => p);
  }, []);

  const body = () => {
    switch (page) {
      case 'home':
        return <Home onGoSearch={() => setPage('search')} />;
      case 'search':
        return <Search />;
      case 'discover':
        return <Discover />;
      case 'library':
        return <Library onLogin={(p) => setLogin(p)} />;
      case 'together':
        return <Together onGoSearch={() => setPage('search')} />;
      case 'settings':
        return <Settings onLogin={(p) => setLogin(p)} onReload={() => setPage('settings')} />;
      default:
        return null;
    }
  };

  return (
    <div className="app">
      <Sidebar page={page} onNavigate={setPage} version={version} />

      <main className="main">
        <div key={page}>{body()}</div>
      </main>

      <Player onOpenQueue={() => setQueueOpen(true)} onOpenNowPlaying={() => setNowOpen(true)} />

      {narrow && (
        <nav className="mobile-tabs">
          {[
            { id: 'home' as PageId, label: '首页', icon: <IconHome size={19} /> },
            { id: 'search' as PageId, label: '搜索', icon: <IconSearch size={19} /> },
            { id: 'library' as PageId, label: '我的', icon: <IconLibrary size={19} /> },
            { id: 'together' as PageId, label: '一起听', icon: <IconUsers size={19} /> },
          ].map((t) => (
            <button
              key={t.id}
              className={`mobile-tab ${page === t.id ? 'active' : ''}`}
              onClick={() => setPage(t.id)}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </nav>
      )}

      <NowPlaying open={nowOpen} onClose={() => setNowOpen(false)} />
      <QueuePanel open={queueOpen} onClose={() => setQueueOpen(false)} />
      <QrLogin
        platform={login ?? 'bilibili'}
        open={login !== null}
        onClose={() => setLogin(null)}
        onSuccess={onLoginSuccess}
      />
      <ToastHost />
    </div>
  );
}
