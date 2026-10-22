import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { fmtTime, trackKey } from '../lib/format';
import { usePlayer } from '../store/player';
import { useTogether, expectedPosition } from '../store/together';
import { toast } from '../components/Toast';
import { IconCrown, IconPlus, IconSend, IconX } from '../components/Icons';
import type { Track } from '../lib/types';

export function Together({ onGoSearch }: { onGoSearch: () => void }) {
  const t = useTogether();
  const [mode, setMode] = useState<'idle' | 'create' | 'join'>('idle');
  const [server, setServer] = useState(t.server || 'http://127.0.0.1:8787');
  const [name, setName] = useState(t.nickname || '听友');
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [chatText, setChatText] = useState('');
  const chatEnd = useRef<HTMLDivElement>(null);

  const localQueue = usePlayer((s) => s.queue);
  const localIndex = usePlayer((s) => s.index);

  useEffect(() => {
    void t.init();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    chatEnd.current?.scrollIntoView({ behavior: 'smooth' });
  }, [t.chats.length]);

  const room = t.room;
  const track: Track | null = room ? room.queue[room.state.index] ?? null : null;
  const isHost = t.isHost();
  const position = room ? expectedPosition(room, t.clockOffset) : 0;

  const doCreate = async () => {
    if (!name.trim()) return toast('先给自己起个名字', 'error');
    setBusy(true);
    try {
      const created = await t.createRoom(server.trim(), `${name.trim()} 的房间`);
      await t.joinRoom(server.trim(), created, name.trim(), '');
      toast(`房间已创建：${created}`);
      setMode('idle');
    } catch (e) {
      toast((e as Error).message, 'error');
    } finally {
      setBusy(false);
    }
  };

  const doJoin = async () => {
    if (!name.trim()) return toast('先给自己起个名字', 'error');
    if (!code.trim()) return toast('请输入房间号', 'error');
    setBusy(true);
    try {
      await t.joinRoom(server.trim(), code.trim(), name.trim(), '');
      setMode('idle');
      toast('已进入房间');
    } catch (e) {
      toast((e as Error).message, 'error');
    } finally {
      setBusy(false);
    }
  };

  const pushLocalQueue = async () => {
    if (!localQueue.length) return toast('本地队列是空的', 'error');
    try {
      await api.togetherSetQueue(localQueue, Math.max(0, localIndex));
      toast(`已把 ${localQueue.length} 首歌推给房间`);
    } catch (e) {
      toast((e as Error).message, 'error');
    }
  };

  const sendChat = async () => {
    const text = chatText.trim();
    if (!text) return;
    setChatText('');
    try {
      await t.sendChat(text);
    } catch (e) {
      toast((e as Error).message, 'error');
    }
  };

  // ---------- 未进房 ----------
  if (!t.joined || !room) {
    return (
      <div>
        <div className="page-head">
          <div>
            <h1 className="page-title">一起听</h1>
            <p className="page-sub">
              和好友同步听 B 站的歌：谁切歌、谁暂停、听到哪儿了，全都实时同步
            </p>
          </div>
        </div>

        <div className="together-layout" style={{ gridTemplateColumns: '1fr' }}>
          <div className="panel" style={{ maxWidth: 620 }}>
            <div style={{ marginBottom: 18 }}>
              <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>
                同步服务器地址
              </label>
              <input
                className="input"
                value={server}
                onChange={(e) => setServer(e.target.value)}
                placeholder="http://127.0.0.1:8787"
              />
              <div style={{ fontSize: 11.5, color: 'var(--text-faint)', marginTop: 7, lineHeight: 1.65 }}>
                在任意一台机器上运行 <code>cargo run --manifest-path server/Cargo.toml</code> 就能开一个房间服务器，
                默认端口 8787。局域网内用 <code>http://你的内网IP:8787</code>，公网部署后填公网地址。
              </div>
            </div>

            <div style={{ marginBottom: 18 }}>
              <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>
                你的昵称
              </label>
              <input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="听友" />
            </div>

            {mode === 'idle' && (
              <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <button className="btn primary" onClick={() => setMode('create')}>
                  <IconPlus size={15} /> 创建房间
                </button>
                <button className="btn" onClick={() => setMode('join')}>
                  输入房间号加入
                </button>
                <button
                  className="btn ghost"
                  onClick={async () => {
                    try {
                      const r = await api.togetherPing(server.trim());
                      toast(r.ok ? '服务器连通 ✅' : '服务器响应异常', r.ok ? 'info' : 'error');
                    } catch (e) {
                      toast((e as Error).message, 'error');
                    }
                  }}
                >
                  测试连通
                </button>
              </div>
            )}

            {mode === 'create' && (
              <div style={{ display: 'flex', gap: 10 }}>
                <button className="btn primary" onClick={doCreate} disabled={busy}>
                  {busy ? '创建中…' : '确认创建'}
                </button>
                <button className="btn ghost" onClick={() => setMode('idle')}>
                  取消
                </button>
              </div>
            )}

            {mode === 'join' && (
              <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                <input
                  className="input"
                  style={{ width: 200 }}
                  value={code}
                  onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                  placeholder="6 位房间号"
                  onKeyDown={(e) => e.key === 'Enter' && doJoin()}
                />
                <button className="btn primary" onClick={doJoin} disabled={busy}>
                  {busy ? '连接中…' : '加入'}
                </button>
                <button className="btn ghost" onClick={() => setMode('idle')}>
                  取消
                </button>
              </div>
            )}

            {t.error && (
              <div style={{ marginTop: 14, color: 'var(--danger)', fontSize: 12.5 }}>{t.error}</div>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ---------- 已进房 ----------
  return (
    <div>
      <div className="page-head">
        <div>
          <h1 className="page-title">一起听</h1>
          <p className="page-sub">
            {t.connected ? '已连接 · 所有操作会同步给房间里的每个人' : '正在重连…'}
          </p>
        </div>
        <div style={{ display: 'flex', gap: 10 }}>
          <button className="btn" onClick={pushLocalQueue}>
            推送我的队列
          </button>
          <button className="btn danger" onClick={() => void t.leave()}>
            退出房间
          </button>
        </div>
      </div>

      <div className="together-layout">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
          <div className="panel">
            <div style={{ display: 'flex', alignItems: 'flex-start', gap: 20, flexWrap: 'wrap' }}>
              <div>
                <div style={{ fontSize: 11.5, color: 'var(--text-faint)', marginBottom: 4 }}>房间号</div>
                <div className="room-code">{room.code}</div>
              </div>
              <div style={{ marginLeft: 'auto', textAlign: 'right' }}>
                <button
                  className="btn"
                  onClick={() => {
                    navigator.clipboard?.writeText(room.code);
                    toast('房间号已复制');
                  }}
                >
                  复制房间号
                </button>
              </div>
            </div>

            <div style={{ marginTop: 20 }}>
              <div style={{ fontSize: 11.5, color: 'var(--text-faint)', marginBottom: 9 }}>
                房间里的人（{room.members.length}）
              </div>
              <div className="members">
                {room.members.map((m) => {
                  const pct = track && track.duration_ms ? (m.position_ms / track.duration_ms) * 100 : 0;
                  return (
                    <div key={m.id} className={`member ${m.is_host ? 'host' : ''}`}>
                      {m.avatar ? (
                        <img className="member-avatar" src={m.avatar} alt="" />
                      ) : (
                        <div className="member-avatar">{m.name.slice(0, 1)}</div>
                      )}
                      <span>{m.name}</span>
                      {m.id === t.selfId && (
                        <span style={{ color: 'var(--text-faint)', fontSize: 11 }}>（我）</span>
                      )}
                      {m.is_host && <IconCrown size={11} className="" />}
                      {m.id !== t.selfId && isHost && (
                        <button
                          className="icon-btn"
                          style={{ width: 18, height: 18 }}
                          title="移出房间"
                          onClick={() => api.togetherKick(m.id).catch((e) => toast(String(e), 'error'))}
                        >
                          <IconX size={12} />
                        </button>
                      )}
                      <div className="member-progress">
                        <i style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} />
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          <div className="panel">
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
              <h3 style={{ margin: 0, fontSize: 15 }}>房间队列</h3>
              <span style={{ fontSize: 12, color: 'var(--text-faint)' }}>{room.queue.length} 首</span>
              <button className="btn ghost" style={{ marginLeft: 'auto' }} onClick={onGoSearch}>
                去搜索加歌
              </button>
            </div>

            {room.queue.length === 0 ? (
              <div className="empty" style={{ padding: '34px 0' }}>
                队列还是空的，去搜索里点歌曲右侧的 ＋ 就能加入房间
              </div>
            ) : (
              <div className="track-list">
                {room.queue.map((tr, i) => (
                  <div
                    key={`${tr.source}-${tr.id}-${i}`}
                    className={`track-row ${i === room.state.index ? 'playing' : ''}`}
                    onDoubleClick={() =>
                      api
                        .togetherPlayIndex(i, 0)
                        .catch((e) => toast(String(e), 'error'))
                    }
                  >
                    <div className="track-index">{i === room.state.index ? '▶' : i + 1}</div>
                    <img className="track-cover" src={tr.cover} alt="" loading="lazy" />
                    <div className="track-main">
                      <div className="track-title">{tr.title}</div>
                      <div className="track-sub">{tr.artist}</div>
                    </div>
                    <div className="track-dur">{fmtTime(tr.duration_ms)}</div>
                    <div className="track-actions" style={{ opacity: 1 }}>
                      <button
                        className="icon-btn"
                        title="从队列移除"
                        onClick={() => api.togetherRemoveTrack(i).catch((e) => toast(String(e), 'error'))}
                      >
                        <IconX size={15} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {track && (
              <div style={{ marginTop: 14, fontSize: 12, color: 'var(--text-faint)' }}>
                正在播放：{track.title} · {fmtTime(position)} / {fmtTime(track.duration_ms)}
                {room.state.by && room.state.by === t.selfId ? ' · 你操作的' : ''}
              </div>
            )}
          </div>
        </div>

        <div className="chat-box">
          <div className="chat-head">房间聊天</div>
          <div className="chat-msgs">
            {t.chats.length === 0 && (
              <div style={{ color: 'var(--text-faint)', fontSize: 12, textAlign: 'center', padding: 20 }}>
                聊点什么吧
              </div>
            )}
            {t.chats.map((c, i) => (
              <div key={i} className={`chat-msg ${c.from === '' ? 'system' : ''}`}>
                <span className="who">{c.from === t.selfId ? '我' : c.name}：</span>
                <span style={{ wordBreak: 'break-word' }}>{c.text}</span>
              </div>
            ))}
            <div ref={chatEnd} />
          </div>
          <div className="chat-input">
            <input
              className="input"
              value={chatText}
              onChange={(e) => setChatText(e.target.value)}
              placeholder="说点什么…"
              onKeyDown={(e) => e.key === 'Enter' && sendChat()}
            />
            <button className="icon-btn" onClick={sendChat}>
              <IconSend size={17} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
