import { useEffect, type ReactNode } from 'react';
import type { PageId, UserInfo } from '../lib/types';
import {
  IconBili,
  IconHome,
  IconLibrary,
  IconMusic,
  IconSearch,
  IconSettings,
  IconUsers,
} from './Icons';
import { useTogether } from '../store/together';
import { useAccount } from '../store/account';

interface Props {
  page: PageId;
  onNavigate: (p: PageId) => void;
  onLogin: () => void;
  version?: string;
}

const MAIN: { id: PageId; label: string; icon: ReactNode }[] = [
  { id: 'home', label: '音乐首页', icon: <IconHome size={18} /> },
  { id: 'search', label: '搜索', icon: <IconSearch size={18} /> },
  { id: 'library', label: '我的音乐', icon: <IconLibrary size={18} /> },
  { id: 'together', label: '一起听', icon: <IconUsers size={18} /> },
];

export function Sidebar({ page, onNavigate, onLogin, version }: Props) {
  const joined = useTogether((s) => s.joined);
  const room = useTogether((s) => s.room);

  const user: UserInfo | null = useAccount((s) => s.user);
  const refresh = useAccount((s) => s.refresh);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <IconMusic size={16} className="" />
        </div>
        <div>
          BiliMusic
          <small>只听歌，不看片</small>
        </div>
      </div>

      <div className="nav-group-label">浏览</div>
      {MAIN.map((item) => (
        <button
          key={item.id}
          className={`nav-item ${page === item.id ? 'active' : ''}`}
          onClick={() => onNavigate(item.id)}
        >
          {item.icon}
          {item.label}
          {item.id === 'together' && joined && <span className="nav-badge">{room?.members.length ?? 0}</span>}
        </button>
      ))}

      <div className="nav-group-label">其他</div>
      <button className={`nav-item ${page === 'settings' ? 'active' : ''}`} onClick={() => onNavigate('settings')}>
        <IconSettings size={18} />
        设置
      </button>

      <div className="account" onClick={() => (user?.logged_in ? onNavigate('settings') : onLogin())}>
        {user?.logged_in ? (
          <>
            {user.avatar ? (
              <img className="account-avatar" src={user.avatar} alt="" referrerPolicy="no-referrer" />
            ) : (
              <div className="account-avatar account-avatar-empty">{(user.name || 'B').slice(0, 1)}</div>
            )}
            <div className="account-meta">
              <div className="account-name">{user.name || '已登录'}</div>
              <div className="account-sub">B 站账号</div>
            </div>
          </>
        ) : (
          <>
            <div className="account-avatar account-avatar-empty">
              <IconBili size={15} />
            </div>
            <div className="account-meta">
              <div className="account-name">登录 B 站</div>
              <div className="account-sub">同步收藏夹 · 稍后再看</div>
            </div>
          </>
        )}
      </div>

      <div className="sidebar-foot">
        <div>纯音频 · 零视频解码</div>
        <div>v{version ?? '0.1.0'}</div>
      </div>
    </aside>
  );
}
