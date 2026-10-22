import { fmtTime } from '../lib/format';
import { api } from '../lib/api';
import { usePlayer } from '../store/player';
import { useTogether } from '../store/together';
import { IconTrash, IconX } from './Icons';
import { toast } from './Toast';

interface Props {
  open: boolean;
  onClose: () => void;
}

export function QueuePanel({ open, onClose }: Props) {
  const queue = usePlayer((s) => s.queue);
  const index = usePlayer((s) => s.index);
  const playAt = usePlayer((s) => s.playAt);
  const removeAt = usePlayer((s) => s.removeAt);
  const clearQueue = usePlayer((s) => s.clearQueue);

  const joined = useTogether((s) => s.joined && s.connected);
  const room = useTogether((s) => s.room);

  const list = joined && room ? room.queue : queue;
  const current = joined && room ? room.state.index : index;

  if (!open) return null;

  return (
    <div className="modal-mask" onMouseDown={onClose}>
      <div
        className="panel"
        style={{ width: 'min(520px, 94vw)', maxHeight: '76vh', display: 'flex', flexDirection: 'column' }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 14 }}>
          <h3 style={{ margin: 0, fontSize: 16 }}>
            播放队列
            <span style={{ color: 'var(--text-faint)', fontSize: 12, fontWeight: 400, marginLeft: 8 }}>
              {list.length} 首{joined ? ' · 一起听' : ''}
            </span>
          </h3>
          <div style={{ marginLeft: 'auto', display: 'flex', gap: 6 }}>
            <button className="btn ghost" onClick={clearQueue} disabled={joined}>
              <IconTrash size={14} /> 清空
            </button>
            <button className="icon-btn" onClick={onClose}>
              <IconX size={18} />
            </button>
          </div>
        </div>

        <div style={{ overflowY: 'auto', flex: 1 }}>
          {list.length === 0 ? (
            <div className="empty" style={{ padding: '40px 0' }}>队列是空的</div>
          ) : (
            list.map((t, i) => (
              <div
                key={`${t.source}-${t.id}-${i}`}
                className={`track-row ${i === current ? 'playing' : ''}`}
                onDoubleClick={() => {
                  if (joined) {
                    void api.togetherPlayIndex(i, 0).catch((e) => toast(String(e), 'error'));
                    return;
                  }
                  playAt(i);
                }}
              >
                <div className="track-index">{i === current ? '▶' : i + 1}</div>
                <img className="track-cover" src={t.cover} alt="" loading="lazy" />
                <div className="track-main">
                  <div className="track-title">{t.title}</div>
                  <div className="track-sub">{t.artist}</div>
                </div>
                <div className="track-dur">{fmtTime(t.duration_ms)}</div>
                <div className="track-actions" style={{ opacity: 1 }}>
                  <button
                    className="icon-btn"
                    title="移除"
                    onClick={() => {
                      if (joined) {
                        void api.togetherRemoveTrack(i).catch((e) => toast(String(e), 'error'));
                        return;
                      }
                      removeAt(i);
                    }}
                  >
                    <IconX size={15} />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
