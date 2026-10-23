import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { trackKey } from '../lib/format';
import type { Track } from '../lib/types';
import { TrackRow } from '../components/TrackRow';
import { useLibrary } from '../store/library';
import { usePlayer } from '../store/player';
import { useAccount } from '../store/account';
import { toast } from '../components/Toast';

interface Folder {
  id: number;
  title: string;
  media_count: number;
}

type Tab = 'fav' | 'recent' | 'bili';

export function Library({ onLogin }: { onLogin: () => void }) {
  const [tab, setTab] = useState<Tab>('fav');
  const [folders, setFolders] = useState<Folder[]>([]);
  const [folderTracks, setFolderTracks] = useState<Track[]>([]);
  const [activeFolder, setActiveFolder] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const account = useAccount((s) => s.user);
  const refreshAccount = useAccount((s) => s.refresh);
  const loggedBili = !!account?.logged_in;

  const favorites = useLibrary((s) => s.favorites);
  const recent = useLibrary((s) => s.recent);
  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const setQueue = usePlayer((s) => s.setQueue);
  const currentKey = queue[index] ? trackKey(queue[index]) : null;

  useEffect(() => {
    void refreshAccount();
  }, [refreshAccount]);

  // 登录态变化后再拉收藏夹，避免"登录成功但列表还是空"
  useEffect(() => {
    if (!loggedBili) {
      setFolders([]);
      return;
    }
    let alive = true;
    api
      .biliFavFolders()
      .then((f) => {
        if (alive) setFolders(f ?? []);
      })
      .catch((e) => toast((e as Error).message, 'error'));
    return () => {
      alive = false;
    };
  }, [loggedBili]);

  // 切收藏夹也是有网络的，连点两个文件夹时旧请求晚回来会串场
  const folderSeq = useRef(0);
  const openFolder = async (f: Folder) => {
    const seq = ++folderSeq.current;
    setActiveFolder(f.id);
    setLoading(true);
    setFolderTracks([]);
    try {
      const res = await api.biliFavList(f.id, 1);
      if (seq !== folderSeq.current) return;
      setFolderTracks(res.items);
    } catch (e) {
      if (seq !== folderSeq.current) return;
      toast((e as Error).message, 'error');
    } finally {
      if (seq === folderSeq.current) setLoading(false);
    }
  };

  const playAll = (list: Track[]) => {
    if (!list.length) return;
    setQueue(list, 0, true);
    toast('已开始播放');
  };

  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title">我的音乐</h1>
          <p className="page-sub">本地收藏 · 最近播放 · B 站收藏夹</p>
        </div>
      </div>

      <div className="chips">
        <button className={`chip ${tab === 'fav' ? 'active' : ''}`} onClick={() => setTab('fav')}>
          本地收藏 {favorites.length}
        </button>
        <button className={`chip ${tab === 'recent' ? 'active' : ''}`} onClick={() => setTab('recent')}>
          最近播放 {recent.length}
        </button>
        <button className={`chip ${tab === 'bili' ? 'active' : ''}`} onClick={() => setTab('bili')}>
          B 站收藏夹
        </button>
      </div>

      {tab === 'fav' && (
        <>
          {favorites.length > 0 && (
            <div style={{ marginBottom: 12 }}>
              <button className="btn primary" onClick={() => playAll(favorites)}>
                播放全部
              </button>
            </div>
          )}
          {favorites.length === 0 ? (
            <div className="empty">
              还没有收藏
              <span style={{ fontSize: 12 }}>在歌曲右侧点 🤍 就能收藏到本地</span>
            </div>
          ) : (
            <div className="track-list">
              {favorites.map((t, i) => (
                <TrackRow key={`${t.source}-${t.id}-${i}`} track={t} index={i} active={currentKey === trackKey(t)} />
              ))}
            </div>
          )}
        </>
      )}

      {tab === 'recent' && (
        <>
          {recent.length === 0 ? (
            <div className="empty">最近还没听过歌</div>
          ) : (
            <div className="track-list">
              {recent.map((t, i) => (
                <TrackRow key={`${t.source}-${t.id}-${i}`} track={t} index={i} active={currentKey === trackKey(t)} />
              ))}
            </div>
          )}
        </>
      )}

      {tab === 'bili' && (
        <>
          {!loggedBili ? (
            <div className="empty">
              登录 B 站后才能同步收藏夹
              <button className="btn primary" onClick={onLogin}>
                扫码登录
              </button>
            </div>
          ) : (
            <>
              <div className="chips">
                {folders.map((f) => (
                  <button
                    key={f.id}
                    className={`chip ${activeFolder === f.id ? 'active' : ''}`}
                    onClick={() => openFolder(f)}
                  >
                    {f.title}（{f.media_count}）
                  </button>
                ))}
              </div>
              {loading ? (
                <div className="empty">
                  <span className="spinner" />
                </div>
              ) : folderTracks.length ? (
                <>
                  <button className="btn primary" style={{ marginBottom: 12 }} onClick={() => playAll(folderTracks)}>
                    播放全部
                  </button>
                  <div className="track-list">
                    {folderTracks.map((t, i) => (
                      <TrackRow key={`${t.source}-${t.id}-${i}`} track={t} index={i} active={currentKey === trackKey(t)} />
                    ))}
                  </div>
                </>
              ) : (
                <div className="empty">选一个收藏夹看看</div>
              )}
            </>
          )}
        </>
      )}

    </div>
  );
}
