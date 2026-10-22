import { useEffect, useState } from 'react';
import { api } from '../lib/api';
import type { Settings as SettingsT } from '../lib/types';
import { toast } from '../components/Toast';

interface Props {
  onLogin: () => void;
  onReload: () => void;
}

export function Settings({ onLogin, onReload }: Props) {
  const [settings, setSettings] = useState<SettingsT | null>(null);
  const [server, setServer] = useState('');
  const [nickname, setNickname] = useState('');
  const [info, setInfo] = useState<{ version: string; proxyPort: number; platform: string } | null>(null);

  const refresh = () => {
    api
      .getSettings()
      .then((s) => {
        setSettings(s);
        setServer(s.togetherServer || 'http://127.0.0.1:8787');
        setNickname(s.nickname);
      })
      .catch((e) => toast((e as Error).message, 'error'));
    api.appInfo().then(setInfo).catch(() => {});
  };

  useEffect(refresh, []);

  const save = async () => {
    try {
      await api.saveSettings(server.trim(), nickname.trim());
      toast('设置已保存');
      onReload();
    } catch (e) {
      toast((e as Error).message, 'error');
    }
  };

  return (
    <div style={{ maxWidth: 720 }}>
      <div className="page-head">
        <div>
          <h1 className="page-title">设置</h1>
          <p className="page-sub">账号 / 一起听服务器 / 关于</p>
        </div>
      </div>

      <div className="panel" style={{ marginBottom: 18 }}>
        <h3 style={{ margin: '0 0 16px', fontSize: 15 }}>账号</h3>

        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 0' }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13.5 }}>B 站账号</div>
            <div style={{ fontSize: 12, color: 'var(--text-faint)', marginTop: 3 }}>
              {settings?.bilibili.loggedIn ? `已登录：${settings.bilibili.name}` : '未登录（不影响听歌）'}
            </div>
          </div>
          {settings?.bilibili.loggedIn ? (
            <button
              className="btn danger"
              onClick={async () => {
                await api.biliLogout();
                toast('已退出 B 站账号');
                refresh();
              }}
            >
              退出登录
            </button>
          ) : (
            <button className="btn primary" onClick={onLogin}>
              扫码登录
            </button>
          )}
        </div>
      </div>

      <div className="panel" style={{ marginBottom: 18 }}>
        <h3 style={{ margin: '0 0 16px', fontSize: 15 }}>一起听</h3>
        <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', marginBottom: 6 }}>
          同步服务器地址
        </label>
        <input className="input" value={server} onChange={(e) => setServer(e.target.value)} placeholder="http://127.0.0.1:8787" />
        <div style={{ fontSize: 11.5, color: 'var(--text-faint)', marginTop: 8, lineHeight: 1.7 }}>
          自建方法：<code>cargo run --release --manifest-path server/Cargo.toml</code>
          <br />
          局域网：填自己电脑的内网 IP，例如 http://192.168.1.10:8787
          <br />
          公网：部署到服务器后用域名 + 反向代理（需要同时转发 WebSocket）。
        </div>

        <label style={{ fontSize: 12, color: 'var(--text-dim)', display: 'block', margin: '16px 0 6px' }}>
          你的昵称
        </label>
        <input className="input" value={nickname} onChange={(e) => setNickname(e.target.value)} placeholder="听友" />

        <div style={{ display: 'flex', gap: 10, marginTop: 18 }}>
          <button className="btn primary" onClick={save}>
            保存
          </button>
          <button
            className="btn"
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
      </div>

      <div className="panel">
        <h3 style={{ margin: '0 0 12px', fontSize: 15 }}>关于</h3>
        <div style={{ fontSize: 12.5, color: 'var(--text-dim)', lineHeight: 1.9 }}>
          <div>BiliMusic · 只用来听歌的 B 站客户端</div>
          <div>版本 {info?.version ?? '-'} · {info?.platform ?? '-'} · 音频代理端口 {info?.proxyPort || '-'}</div>
          <div style={{ marginTop: 10, color: 'var(--text-faint)' }}>
            本应用只取视频的音频轨道，不加载任何画面，流量和内存都只有原视频的零头。
          </div>
        </div>
      </div>
    </div>
  );
}
