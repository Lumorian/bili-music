import { useEffect, useState } from 'react';
import { audioSupported, guardSeek, getAudio } from '../lib/audio';
import { fmtTime } from '../lib/format';
import type { Track } from '../lib/types';
import { useLibrary } from '../store/library';
import { usePlayer } from '../store/player';
import { useTogether, expectedPosition } from '../store/together';
import { api } from '../lib/api';
import {
  IconHeart,
  IconList,
  IconMic,
  IconNext,
  IconPause,
  IconPlay,
  IconPrev,
  IconRepeat,
  IconRepeatOne,
  IconShuffle,
  IconVolume,
  IconVolumeMute,
} from './Icons';
import { Slider } from './Slider';
import { toast } from './Toast';

interface Props {
  onOpenQueue: () => void;
  onOpenNowPlaying: () => void;
}

export function Player({ onOpenQueue, onOpenNowPlaying }: Props) {
  const p = usePlayer();
  const track: Track | null = p.queue[p.index] ?? null;
  const fav = useLibrary((s) => (track ? s.isFavorite(track) : false));
  const toggleFavorite = useLibrary((s) => s.toggleFavorite);

  const joined = useTogether((s) => s.joined && s.connected);
  const room = useTogether((s) => s.room);
  const isHost = useTogether((s) => s.isHost());

  // 一起听时进度由「服务器状态 + 时钟偏移」推算，需要一个心跳来触发重渲染
  const [, setBeat] = useState(0);
  useEffect(() => {
    if (!joined) return;
    const id = window.setInterval(() => setBeat((n) => n + 1), 500);
    return () => window.clearInterval(id);
  }, [joined]);

  // 一起听时进度条显示"服务器时间对应的位置"
  const syncDuration = joined && room ? room.queue[room.state.index]?.duration_ms ?? 0 : 0;
  const displayPos = joined && room ? expectedPosition(room, useTogether.getState().clockOffset) : p.position;
  const displayDur = joined && room ? syncDuration : p.duration || track?.duration_ms || 0;

  const ctrl = {
    toggle: () => {
      if (joined) {
        // 以房间状态为准：本地 p.playing 可能还没跟上服务器广播，
        // 用它取反会出现"点一下没反应、再点还是没反应"。
        const roomPlaying = room?.state.playing ?? p.playing;
        const pos = Math.round(displayPos);
        void api.togetherSetPlaying(!roomPlaying, pos).catch((e) => toast(String(e), 'error'));
        return;
      }
      p.toggle();
    },
    next: () => {
      if (joined) {
        void api.togetherNext().catch((e) => toast(String(e), 'error'));
        return;
      }
      p.next();
    },
    prev: () => {
      if (joined) {
        void api.togetherPrev().catch((e) => toast(String(e), 'error'));
        return;
      }
      p.prev();
    },
    seek: (v: number) => {
      if (joined) {
        // 等服务器回声回来前别纠偏，否则手一松就被拉回原位
        guardSeek();
        void api.togetherSeek(Math.round(v)).catch((e) => toast(String(e), 'error'));
        return;
      }
      p.seek(v);
      guardSeek(600);
      // 没有音频后端时连 audio 元素都不要碰（赋值 src 会让 WebProcess 崩）
      if (!audioSupported()) return;
      const a = getAudio();
      if (a.readyState > 0) a.currentTime = v / 1000;
    },
  };

  const modeIcon = () => {
    if (p.mode === 'repeat') return <IconRepeat size={17} />;
    if (p.mode === 'single') return <IconRepeatOne size={17} />;
    if (p.mode === 'shuffle') return <IconShuffle size={17} />;
    return <IconList size={17} />;
  };

  return (
    <div className="player">
      <div className="player-left">
        {track ? (
          <>
            <img
              className="player-cover"
              src={track.cover}
              alt=""
              onClick={onOpenNowPlaying}
              draggable={false}
              referrerPolicy="no-referrer"
            />
            <div className="player-meta">
              <div className="player-title">{track.title}</div>
              <div className="player-artist">
                {joined ? '一起听 · ' : ''}
                {track.artist}
              </div>
            </div>
            <button
              className={`icon-btn ${fav ? 'active' : ''}`}
              onClick={() => toggleFavorite(track)}
              title="收藏"
            >
              <IconHeart size={16} filled={fav} />
            </button>
          </>
        ) : (
          <div className="player-meta">
            <div className="player-title">还没在听歌</div>
            <div className="player-artist">从左侧挑一首开始吧</div>
          </div>
        )}
      </div>

      <div className="player-center">
        <div className="player-controls">
          <button
            className="icon-btn"
            onClick={p.cycleMode}
            title={p.mode === 'order' ? '顺序播放' : p.mode === 'repeat' ? '列表循环' : p.mode === 'single' ? '单曲循环' : '随机播放'}
            disabled={joined}
          >
            {modeIcon()}
          </button>
          <button className="icon-btn" onClick={ctrl.prev}>
            <IconPrev size={19} />
          </button>
          <button className="play-btn" onClick={ctrl.toggle} disabled={!track && !joined}>
            {p.loading ? (
              <span className="spinner" />
            ) : (joined ? (room?.state.playing ?? p.playing) : p.playing) ? (
              <IconPause size={18} />
            ) : (
              <IconPlay size={18} />
            )}
          </button>
          <button className="icon-btn" onClick={ctrl.next}>
            <IconNext size={19} />
          </button>
          <button
            className={`icon-btn ${joined ? 'active' : ''}`}
            onClick={onOpenQueue}
            title="播放队列"
          >
            <IconList size={18} />
          </button>
        </div>
        <div className="player-progress">
          <span>{fmtTime(displayPos)}</span>
          <Slider value={displayPos} max={displayDur} onCommit={ctrl.seek} />
          <span>{fmtTime(displayDur)}</span>
        </div>
      </div>

      <div className="player-right">
        {joined && (
          <span className="room-tag" style={{ marginRight: 4 }}>
            <IconMic size={10} /> {isHost ? '房主' : '同步中'}
          </span>
        )}
        <div className="volume-wrap">
          <button className="icon-btn" onClick={p.toggleMute} title="静音">
            {p.muted || p.volume === 0 ? <IconVolumeMute size={17} /> : <IconVolume size={17} />}
          </button>
          <Slider value={p.muted ? 0 : p.volume * 100} max={100} onChange={(v) => p.setVolume(v / 100)} />
        </div>
        <button className="icon-btn" onClick={onOpenQueue} title="队列">
          <IconList size={18} />
        </button>
      </div>
    </div>
  );
}
