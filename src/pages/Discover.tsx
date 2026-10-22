import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import { trackKey } from '../lib/format';
import type { Track } from '../lib/types';
import { TrackRow } from '../components/TrackRow';
import { toast } from '../components/Toast';
import { usePlayer } from '../store/player';

interface Toplist {
  name: string;
  id: string;
}

/** 网易云榜单入口：热歌榜、飙升榜这类，适合当作"电台"随便听听 */
export function Discover() {
  const [lists, setLists] = useState<Toplist[]>([]);
  const [activeId, setActiveId] = useState('');
  const [title, setTitle] = useState('');
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const setQueue = usePlayer((s) => s.setQueue);
  const currentKey = queue[index] ? trackKey(queue[index]) : null;

  useEffect(() => {
    let alive = true;
    api
      .neToplists()
      .then((list) => {
        if (!alive) return;
        setLists(list);
        if (list.length) void load(list[0].id, list[0].name);
      })
      .catch((e) => alive && setError((e as Error).message))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const load = async (id: string, name: string) => {
    setActiveId(id);
    setTitle(name);
    setLoading(true);
    setError('');
    try {
      const res = await api.nePlaylist(id);
      setTracks(res.tracks);
    } catch (e) {
      setError((e as Error).message);
      setTracks([]);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title">网易云榜单</h1>
          <p className="page-sub">不用想听什么的时候，点开一个榜单当电台</p>
        </div>
        <button
          className="btn primary"
          onClick={() => {
            if (!tracks.length) return;
            setQueue(tracks, 0, true);
            toast(`播放「${title}」`);
          }}
          disabled={!tracks.length}
        >
          全部播放
        </button>
      </div>

      <div className="chips">
        {lists.map((l) => (
          <button
            key={l.id}
            className={`chip ${activeId === l.id ? 'active' : ''}`}
            onClick={() => load(l.id, l.name)}
          >
            {l.name}
          </button>
        ))}
      </div>

      {loading ? (
        <div className="empty">
          <span className="spinner" /> 加载中…
        </div>
      ) : error ? (
        <div className="empty">{error}</div>
      ) : (
        <div className="track-list">
          {tracks.map((t, i) => (
            <TrackRow key={`${t.source}-${t.id}-${i}`} track={t} index={i} active={currentKey === trackKey(t)} />
          ))}
        </div>
      )}
    </div>
  );
}
