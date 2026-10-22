import { useCallback, useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type { Source, Track } from '../lib/types';
import { trackKey } from '../lib/format';
import { usePlayer } from '../store/player';
import { TrackRow } from '../components/TrackRow';
import { toast } from '../components/Toast';

const HOT = ['周杰伦', 'VOCALOID', '纯音乐', '轻音乐 学习', '中文说唱', '古典音乐', '洛天依', '游戏BGM'];

export function Search({ initialSource }: { initialSource?: Source }) {
  const [source, setSource] = useState<Source>(initialSource ?? 'bilibili');
  const [keyword, setKeyword] = useState('');
  const [submitted, setSubmitted] = useState('');
  const [tracks, setTracks] = useState<Track[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const setQueue = usePlayer((s) => s.setQueue);
  const currentKey = queue[index] ? trackKey(queue[index]) : null;

  const run = useCallback(
    async (kw: string, pg: number, replace: boolean) => {
      if (!kw.trim()) return;
      setLoading(true);
      setError('');
      try {
        const res =
          source === 'bilibili' ? await api.biliSearch(kw, pg) : await api.neSearch(kw, pg);
        setTracks((prev) => (replace ? res.items : [...prev, ...res.items]));
        setHasMore(res.has_more);
        setPage(pg);
      } catch (e) {
        setError((e as Error).message);
      } finally {
        setLoading(false);
      }
    },
    [source],
  );

  const submit = (kw = keyword) => {
    const clean = kw.trim();
    if (!clean) return;
    setSubmitted(clean);
    setKeyword(clean);
    void run(clean, 1, true);
  };

  useEffect(() => {
    // 切换音源后自动用同一关键词重搜一次，体验更顺
    if (submitted) void run(submitted, 1, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source]);

  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title">搜索</h1>
          <p className="page-sub">同时支持 B 站视频音频与网易云曲库</p>
        </div>
      </div>

      <div className="search-bar">
        <div className="source-switch">
          <button className={source === 'bilibili' ? 'active' : ''} onClick={() => setSource('bilibili')}>
            B 站
          </button>
          <button className={source === 'netease' ? 'active' : ''} onClick={() => setSource('netease')}>
            网易云
          </button>
        </div>
        <input
          ref={inputRef}
          className="input"
          placeholder={source === 'bilibili' ? '搜视频 / UP 主 / 歌名…' : '搜歌曲 / 歌手 / 专辑…'}
          value={keyword}
          onChange={(e) => setKeyword(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && submit()}
        />
        <button className="btn primary" onClick={() => submit()} disabled={loading}>
          搜索
        </button>
      </div>

      {!submitted && (
        <div className="chips">
          {HOT.map((h) => (
            <button key={h} className="chip" onClick={() => submit(h)}>
              {h}
            </button>
          ))}
        </div>
      )}

      {submitted && tracks.length > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 12, gap: 10 }}>
          <span style={{ color: 'var(--text-dim)', fontSize: 13 }}>
            「{submitted}」共 {tracks.length} 条结果
          </span>
          <button
            className="btn"
            style={{ marginLeft: 'auto' }}
            onClick={() => {
              setQueue(tracks, 0, true);
              toast('已开始播放');
            }}
          >
            全部播放
          </button>
        </div>
      )}

      {error && <div className="empty">{error}</div>}

      {loading && !tracks.length ? (
        <div className="empty">
          <span className="spinner" />
          搜索中…
        </div>
      ) : (
        <div className="track-list">
          {tracks.map((t, i) => (
            <TrackRow key={`${t.source}-${t.id}-${i}`} track={t} index={i} active={currentKey === trackKey(t)} />
          ))}
        </div>
      )}

      {submitted && !loading && !tracks.length && !error && (
        <div className="empty">没找到相关内容，换个关键词试试</div>
      )}

      {hasMore && (
        <div style={{ display: 'flex', justifyContent: 'center', marginTop: 20 }}>
          <button className="btn" onClick={() => run(submitted, page + 1, false)} disabled={loading}>
            {loading ? '加载中…' : '加载更多'}
          </button>
        </div>
      )}
    </div>
  );
}
