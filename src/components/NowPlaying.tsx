import { useEffect, useMemo, useState } from 'react';
import { api } from '../lib/api';
import { fmtTime } from '../lib/format';
import type { TrackDetail } from '../lib/types';
import { usePlayer } from '../store/player';
import { useTogether, expectedPosition } from '../store/together';
import { IconChevronDown, IconHeart } from './Icons';
import type { Track } from '../lib/types';
import { useLibrary } from '../store/library';

interface Props {
  open: boolean;
  onClose: () => void;
}

/**
 * 全屏"正在播放"页。
 * B 站视频基本没有歌词接口，这里展示的是视频简介 + 分 P 列表，
 * 用来替代歌词位置，既符合"只听"的定位，也不假装有歌词。
 */
export function NowPlaying({ open, onClose }: Props) {
  const p = usePlayer();
  const track = p.queue[p.index] ?? null;
  const [detail, setDetail] = useState<TrackDetail | null>(null);
  const [loading, setLoading] = useState(false);

  const joined = useTogether((s) => s.joined && s.connected);
  const room = useTogether((s) => s.room);
  const fav = useLibrary((s) => (track ? s.isFavorite(track) : false));
  const toggleFavorite = useLibrary((s) => s.toggleFavorite);

  const [, setBeat] = useState(0);
  useEffect(() => {
    if (!open || !joined) return;
    const id = window.setInterval(() => setBeat((n) => n + 1), 500);
    return () => window.clearInterval(id);
  }, [open, joined]);

  const position = joined && room ? expectedPosition(room, useTogether.getState().clockOffset) : p.position;

  useEffect(() => {
    if (!open || !track) return;
    let alive = true;
    setDetail(null);
    setLoading(true);
    api
      .biliDetail(track.id)
      .then((d) => {
        if (alive) setDetail(d);
      })
      .catch(() => {
        /* 详情拿不到就算了，页面其它部分照常显示 */
      })
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [open, track?.id]);

  const chapters = useMemo(() => detail?.pages ?? [], [detail]);

  // 当前播的是第几个分 P（track.extra 就是 cid；没写说明还没解析过，按第一 P 算）
  const currentCid = track?.extra ? Number(track.extra) : chapters[0]?.[0] ?? 0;
  const queueIndex = usePlayer((s) => s.index);

  /** 切到某个分 P：把 cid 写回队列里这条 track，然后重新解析音轨 */
  const playPart = (cid: number) => {
    // 一起听模式下播放进度归房间管，本地私自换分 P 会立刻被广播打回去
    if (joined) return;
    if (!track || cid === currentCid) return;
    const patch: Track = { ...track, extra: String(cid), duration_ms: (chapters.find(([c]) => c === cid)?.[2] ?? 0) * 1000 || track.duration_ms };
    const q = [...p.queue];
    q[queueIndex] = patch;
    p.setQueue(q, queueIndex, false);
    p.playAt(queueIndex, 0);
  };

  if (!open) return null;

  return (
    <div className="now-playing">
      <div className="now-head">
        <button className="icon-btn" onClick={onClose} title="收起">
          <IconChevronDown size={20} />
        </button>
        <div style={{ fontSize: 13, color: 'var(--text-dim)' }}>正在播放</div>
        <div style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-faint)' }}>
          B 站纯音频
        </div>
      </div>

      <div className="now-body">
        <div className="now-art">
          <img className="now-cover" src={track?.cover} alt="" draggable={false} referrerPolicy="no-referrer" />
          <div>
            <h1 className="now-title">{track?.title ?? '未在播放'}</h1>
            <p className="now-artist">
              {track?.artist}
              {track?.subtitle ? ` · ${track.subtitle}` : ''}
            </p>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 14 }}>
              <button
                className={`icon-btn ${fav ? 'active' : ''}`}
                onClick={() => track && toggleFavorite(track)}
                title="收藏"
              >
                <IconHeart size={18} filled={fav} />
              </button>
              <span style={{ fontSize: 12, color: 'var(--text-faint)' }}>
                {fmtTime(position)} / {fmtTime(p.duration || track?.duration_ms || 0)}
              </span>
            </div>
          </div>
        </div>

        <div className="lyrics">
          {loading ? (
            <div className="empty" style={{ padding: 0 }}>
              <span className="spinner" />
            </div>
          ) : (
            <>
              {chapters.length > 1 && (
                <>
                  <div className="now-section">分 P（共 {chapters.length} 段）</div>
                  {chapters.map(([cid, part, dur], i) => (
                    <div
                      key={cid}
                      className={`now-paragraph now-chapter ${cid === currentCid ? 'active' : ''}`}
                      onClick={() => playPart(cid)}
                      title={joined ? '一起听模式下由房主决定播放哪一段' : '切换到这一段'}
                    >
                      <span className="now-chapter-idx">{String(i + 1).padStart(2, '0')}</span>
                      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>
                        {part || `P${i + 1}`}
                      </span>
                      <span style={{ color: 'var(--text-faint)', fontSize: 12, flex: 'none' }}>
                        {fmtTime(dur * 1000)}
                      </span>
                    </div>
                  ))}
                  <div style={{ height: 22 }} />
                </>
              )}
              {detail?.desc ? (
                <>
                  <div className="now-section">视频简介</div>
                  <div className="now-paragraph now-desc">{detail.desc}</div>
                </>
              ) : (
                <div className="empty" style={{ padding: 0 }}>
                  戴上耳机听吧 🎧
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
