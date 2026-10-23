import { fmtTime } from '../lib/format';
import type { Track } from '../lib/types';
import { useLibrary } from '../store/library';
import { usePlayer } from '../store/player';
import { useTogether } from '../store/together';
import { api } from '../lib/api';
import { IconHeart, IconPlus, IconX } from './Icons';
import { toast } from './Toast';

interface Props {
  track: Track;
  index?: number;
  /** 当前是否正在播这首 */
  active?: boolean;
  onRemove?: () => void;
  /** 显示删除按钮而不是收藏按钮（用于队列/房间列表） */
  removable?: boolean;
}

export function TrackRow({ track, index, active, onRemove, removable }: Props) {
  const { playTrack, enqueue } = usePlayer();
  const { isFavorite, toggleFavorite } = useLibrary();
  const joined = useTogether((s) => s.joined && s.connected);
  const fav = isFavorite(track);

  const play = () => {
    if (joined) {
      // 一起听：把这首歌交给房间，让所有人一起听
      void api
        .togetherAddTrack(track, true)
        .then(() => toast('已加入一起听队列'))
        .catch((e) => toast(String(e), 'error'));
      return;
    }
    playTrack(track);
  };

  return (
    <div className={`track-row ${active ? 'playing' : ''}`} onDoubleClick={play}>
      <div className="track-index">
        {active ? (
          <span style={{ color: 'var(--accent)' }}>▶</span>
        ) : index != null ? (
          index + 1
        ) : (
          ''
        )}
      </div>
      <img
        className="track-cover"
        src={track.cover}
        alt=""
        loading="lazy"
        draggable={false}
        referrerPolicy="no-referrer"
      />
      <div className="track-main">
        <div className="track-title">
          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{track.title}</span>
        </div>
        <div className="track-sub">
          {track.artist}
          {track.album ? ` · ${track.album}` : ''}
          {track.subtitle ? ` · ${track.subtitle}` : ''}
        </div>
      </div>
      <div className="track-dur">{fmtTime(track.duration_ms)}</div>
      <div className="track-actions">
        <button className="icon-btn" onClick={play} title="播放">
          <IconPlus size={15} />
        </button>
        {removable ? (
          <button className="icon-btn" onClick={onRemove} title="移除">
            <IconX size={15} />
          </button>
        ) : (
          <button
            className={`icon-btn ${fav ? 'active' : ''}`}
            onClick={() => toggleFavorite(track)}
            title={fav ? '取消收藏' : '收藏'}
          >
            <IconHeart size={15} filled={fav} />
          </button>
        )}
      </div>
    </div>
  );
}

export function TrackList({ tracks, currentKey }: { tracks: Track[]; currentKey?: string | null }) {
  if (!tracks.length) {
    return <div className="empty">这里还没有内容</div>;
  }
  return (
    <div className="track-list">
      {tracks.map((t, i) => (
        <TrackRow
          key={`${t.source}-${t.id}-${i}`}
          track={t}
          index={i}
          active={currentKey === `${t.source}:${t.id}`}
        />
      ))}
    </div>
  );
}
