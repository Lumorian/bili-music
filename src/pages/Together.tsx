import { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { fmtTime } from '../lib/format';
import { usePlayer } from '../store/player';
import { useTogether, expectedPosition, type TogetherMode } from '../store/together';
import { toast } from '../components/Toast';
import { IconCrown, IconPlus, IconSend, IconX } from '../components/Icons';
import type { Track } from '../lib/types';

type Step = 'idle' | 'create' | 'join';

export function Together({ onGoSearch }: { onGoSearch: () => void }) {
  const t = useTogether();
  const [step, setStep] = useState<Step>('idle');
  const [name, setName] = useState(t.nickname || '听友');
  const [code, setCode] = useState('');
  const [busy, setBusy] = useState(false);
  const [chatText, setChatText] = useState('');
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [serverInput, setServerInput] = useState(t.server || 'http://127.0.0.1:8787');
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

  /** 建房：云模式不需要服务器；自建模式需要先起服务器 */
  const doCreate = async (mode: TogetherMode) => {
    const nick = name.trim();
    if (!nick) return toast('先给自己起个名字', 'error');
    setBusy(true);
    try {
      const created =
        mode === 'cloud'
          ? await t.createCloud(nick)
          : await t.createServer(serverInput.trim() || 'http://127.0.0.1:8787', `${nick} 的房间`);
      toast(`房间已创建：${created}`);
      setStep('idle');
    } catch (e) {
      toast((e as Error).message, 'error');
    } finally {
      setBusy(false);
    }
  };

  const doJoin = async (mode: TogetherMode) => {
    const nick = name.trim();
    const roomCode = code.trim();
    if (!nick) return toast('先给自己起个名字', 'error');
    if (!roomCode) return toast('请输入房间号', 'error');
    setBusy(true);
    try {
      if (mode === 'cloud') {
        await t.joinCloud(roomCode, nick);
      } else {
        await t.joinServer(serverInput.trim() || 'http://127.0.0.1:8787', roomCode, nick);
      }
      setStep('idle');
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

        <div className="panel" style={{ maxWidth: 640, marginBottom: 18 }}>
          <div style={{ marginBottom: 18 }}>
            <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>
              你的昵称
            </label>
            <input
              className="input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="听友"
            />
          </div>

          <div style={{
            padding: '14px 16px',
            borderRadius: 'var(--radius-sm)',
            background: 'rgba(255,107,166,0.08)',
            border: '1px solid rgba(255,107,166,0.25)',
            marginBottom: 18,
            fontSize: 12.5,
            lineHeight: 1.75,
            color: 'var(--text-dim)',
          }}>
            <strong style={{ color: 'var(--accent)' }}>不用自建服务器。</strong>
            点「创建房间」会直接分配一个 6 位房间号，把号发给朋友，他们在同一页输入即可。
            同步走公共 MQTT 通道，房主的客户端代行服务器职责。
          </div>

          {step === 'idle' && (
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
              <button className="btn primary" onClick={() => void doCreate('cloud')} disabled={busy}>
                <IconPlus size={15} /> {busy ? '创建中…' : '创建房间'}
              </button>
              <button className="btn" onClick={() => setStep('join')} disabled={busy}>
                输入房间号加入
              </button>
              <button
                className="btn ghost"
                onClick={async () => {
                  try {
                    const r = await api.togetherCloudProbe(t.broker.trim() || undefined);
                    toast(r.ok ? `公共通道可用：${r.broker}` : r.error ?? '连接失败', r.ok ? 'info' : 'error');
                  } catch (e) {
                    toast((e as Error).message, 'error');
                  }
                }}
              >
                测速
              </button>
            </div>
          )}

          {step === 'join' && (
            <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap', alignItems: 'center' }}>
              <input
                className="input"
                style={{ width: 200 }}
                value={code}
                onChange={(e) => setCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                placeholder="6 位房间号"
                onKeyDown={(e) => e.key === 'Enter' && void doJoin('cloud')}
              />
              <button className="btn primary" onClick={() => void doJoin('cloud')} disabled={busy}>
                {busy ? '连接中…' : '加入'}
              </button>
              <button className="btn ghost" onClick={() => setStep('idle')}>
                取消
              </button>
            </div>
          )}

          {t.error && (
            <div style={{ marginTop: 14, color: 'var(--danger)', fontSize: 12.5 }}>{t.error}</div>
          )}

          <button
            className="btn ghost"
            style={{ marginTop: 20, fontSize: 12, color: 'var(--text-faint)' }}
            onClick={() => setShowAdvanced((v) => !v)}
          >
            {showAdvanced ? '收起' : '高级：改用自建服务器'}
          </button>

          {showAdvanced && (
            <div style={{ marginTop: 14, paddingTop: 14, borderTop: '1px solid var(--line)' }}>
              <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>
                自建服务器地址
              </label>
              <input
                className="input"
                value={serverInput}
                onChange={(e) => setServerInput(e.target.value)}
                placeholder="http://127.0.0.1:8787"
              />
              <div style={{ fontSize: 11.5, color: 'var(--text-faint)', marginTop: 7, lineHeight: 1.7 }}>
                想在局域网 / 公网更稳定（不受第三方公共通道影响），可以自己跑一个房间服务器：
                <br />
                <code>cargo run --release --manifest-path server/Cargo.toml</code>
                <br />
                然后在 App 里填 <code>http://你的IP:8787</code>。两种模式都能和好友互通的前提是大家对同一个地址。
              </div>
            </div>
          )}
        </div>

        <div className="panel" style={{ maxWidth: 640, fontSize: 12.5, color: 'var(--text-faint)', lineHeight: 1.9 }}>
          <div style={{ color: 'var(--text-dim)', fontWeight: 600, marginBottom: 8 }}>这套方案的前提</div>
          <div>· 公共通道是明文话题，猜到房间号的人理论上能监听，别在房间里聊隐私。</div>
          <div>· 房主是「服务器」：房主关掉 App，房间就散了。</div>
          <div>· 想完全自己掌控，用高级里的自建服务器模式。</div>
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
            <span style={{ marginLeft: 10, color: 'var(--text-faint)', fontSize: 12 }}>
              {t.mode === 'cloud' ? '云房间 · 走公共通道，无需服务器' : '自建服务器模式'}
            </span>
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
                        <img className="member-avatar" src={m.avatar} alt="" referrerPolicy="no-referrer" />
                      ) : (
                        <div className="member-avatar">{m.name.slice(0, 1)}</div>
                      )}
                      <span>{m.name}</span>
                      {m.id === t.selfId && (
                        <span style={{ color: 'var(--text-faint)', fontSize: 11 }}>（我）</span>
                      )}
                      {m.is_host && <IconCrown size={11} className="" />}
                      {m.id !== t.selfId && isHost && (
                        <>
                          <button
                            className="icon-btn"
                            style={{ width: 18, height: 18 }}
                            title="把房主交给他"
                            onClick={() =>
                              api
                                .togetherTransferHost(m.id)
                                .then(() => toast(`已把房主交给 ${m.name}`))
                                .catch((e) => toast(String(e), 'error'))
                            }
                          >
                            <IconCrown size={11} />
                          </button>
                          <button
                            className="icon-btn"
                            style={{ width: 18, height: 18 }}
                            title="移出房间"
                            onClick={() => api.togetherKick(m.id).catch((e) => toast(String(e), 'error'))}
                          >
                            <IconX size={12} />
                          </button>
                        </>
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
                    <img className="track-cover" src={tr.cover} alt="" loading="lazy" referrerPolicy="no-referrer" />
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
