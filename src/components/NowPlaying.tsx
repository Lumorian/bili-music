import { useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../lib/api';
import { fmtTime, parseLrc, type LyricLine } from '../lib/format';
import { usePlayer } from '../store/player';
import { useTogether, expectedPosition } from '../store/together';
import { IconChevronDown, IconHeart } from './Icons';
import { useLibrary } from '../store/library';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function NowPlaying({ open, onClose }: Props) {
  const p = usePlayer();
  const track = p.queue[p.index] ?? null;
  const [lines, setLines] = useState<LyricLine[]>([]);
  const [active, setActive] = useState(-1);
  const listRef = useRef<HTMLDivElement>(null);

  const joined = useTogether((s) => s.joined && s.connected);
  const room = useTogether((s) => s.room);
  const fav = useLibrary((s) => (track ? s.isFavorite(track) : false));
  const toggleFavorite = useLibrary((s) => s.toggleFavorite);

  const position = joined && room ? expectedPosition(room, useTogether.getState().clockOffset) : p.position;

  useEffect(() => {
    if (!open || !track) return;
    let alive = true;
    setLines([]);
    // B 站没有可用歌词接口（多数视频没有字幕），只有网易云有 LRC
    if (track.source !== 'netease') return;
    api
      .neLyric(track.id)
      .then(({ lrc, translation }) => {
        if (!alive) return;
        const main = parseLrc(lrc);
        const trans = parseLrc(translation);
        const merged = main.map((l) => {
          const t = trans.find((x) => Math.abs(x.time - l.time) < 700);
          return t && t.text !== l.text ? { ...l, text: `${l.text}` } : l;
        });
        setLines(merged);
      })
      .catch(() => setLines([]));
    return () => {
      alive = false;
    };
  }, [open, track?.source, track?.id]);

  useEffect(() => {
    if (!lines.length) return;
    let idx = -1;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].time <= position) idx = i;
      else break;
    }
    setActive(idx);
  }, [position, lines]);

  useEffect(() => {
    if (active < 0 || !listRef.current) return;
    const el = listRef.current.querySelector<HTMLElement>(`[data-lyric="${active}"]`);
    el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }, [active]);

  if (!open) return null;

  return (
    <div className="now-playing">
      <div className="now-head">
        <button className="icon-btn" onClick={onClose} title="收起">
          <IconChevronDown size={20} />
        </button>
        <div style={{ fontSize: 13, color: 'var(--text-dim)' }}>正在播放</div>
        <div style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text-faint)' }}>
          {track?.source === 'bilibili' ? 'B 站音频' : track?.source === 'netease' ? '网易云音乐' : ''}
        </div>
      </div>

      <div className="now-body">
        <div className="now-art">
          <img className="now-cover" src={track?.cover} alt="" draggable={false} />
          <div>
            <h1 className="now-title">{track?.title ?? '未在播放'}</h1>
            <p className="now-artist">
              {track?.artist}
              {track?.album ? ` · ${track.album}` : ''}
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

        <div className="lyrics" ref={listRef}>
          {lines.length === 0 ? (
            <div className="empty" style={{ padding: 0 }}>
              {track?.source === 'bilibili'
                ? 'B 站的视频一般没有歌词，戴上耳机听吧 🎧'
                : '暂无歌词'}
            </div>
          ) : (
            lines.map((l, i) => (
              <div
                key={`${l.time}-${i}`}
                data-lyric={i}
                className={`lyric-line ${i === active ? 'active' : i < active ? 'passed' : ''}`}
                onClick={() => {
                  usePlayer.getState().seek(l.time);
                  import('../lib/audio').then((m) => {
                    m.guardSeek(600);
                    const a = m.getAudio();
                    if (a.readyState > 0) a.currentTime = l.time / 1000;
                  });
                }}
              >
                {l.text}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
