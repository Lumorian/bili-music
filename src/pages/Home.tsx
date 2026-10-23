import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import type { Track } from '../lib/types';
import { TrackList } from '../components/TrackRow';
import { usePlayer } from '../store/player';
import { useLibrary } from '../store/library';
import { trackKey } from '../lib/format';
import { toast } from '../components/Toast';
import { IconMusic } from '../components/Icons';

interface Region {
  name: string;
  id: number;
}

const REGIONS: Region[] = [
  { name: '音乐区', id: 3 },
  { name: 'VOCALOID', id: 30 },
  { name: '演奏', id: 59 },
  { name: '翻唱', id: 130 },
  { name: '原创音乐', id: 28 },
  { name: '音乐现场', id: 31 },
  { name: 'MV', id: 193 },
  { name: '国风音乐', id: 168 },
];

export function Home({ onGoSearch }: { onGoSearch: () => void }) {
  const [region, setRegion] = useState(3);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const setQueue = usePlayer((s) => s.setQueue);
  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const recent = useLibrary((s) => s.recent);

  const currentKey = queue[index] ? trackKey(queue[index]) : null;

  // 切分区时旧榜单晚回来会覆盖新分区的结果
  const seqRef = useRef(0);
  useEffect(() => {
    const seq = ++seqRef.current;
    setLoading(true);
    setError('');
    api
      .biliRanking(region, 1)
      .then((page) => {
        if (seq === seqRef.current) setTracks(page.items);
      })
      .catch((e) => {
        if (seq === seqRef.current) setError((e as Error).message);
      })
      .finally(() => {
        if (seq === seqRef.current) setLoading(false);
      });
  }, [region]);

  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title">音乐首页</h1>
          <p className="page-sub">B 站音乐分区排行 · 只取音轨，不占带宽看视频</p>
        </div>
        <button
          className="btn primary"
          onClick={() => {
            if (!tracks.length) return;
            setQueue(tracks, 0, true);
            toast(`开始播放「${tracks[0].title}」`);
          }}
          disabled={!tracks.length}
        >
          <IconMusic size={15} /> 全部播放
        </button>
      </div>

      <div className="chips">
        {REGIONS.map((r) => (
          <button
            key={r.id}
            className={`chip ${region === r.id ? 'active' : ''}`}
            onClick={() => setRegion(r.id)}
          >
            {r.name}
          </button>
        ))}
      </div>

      {loading ? (
        <div className="empty">
          <span className="spinner" />
          正在拉取榜单…
        </div>
      ) : error ? (
        <div className="empty">
          {error}
          <button className="btn" onClick={onGoSearch}>
            去搜索
          </button>
        </div>
      ) : (
        <TrackList tracks={tracks} currentKey={currentKey} />
      )}

      {recent.length > 0 && (
        <>
          <h2 className="page-title" style={{ fontSize: 18, marginTop: 34, marginBottom: 12 }}>
            最近听过
          </h2>
          <TrackList tracks={recent.slice(0, 8)} currentKey={currentKey} />
        </>
      )}
    </div>
  );
}
