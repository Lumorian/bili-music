import type { ReactNode } from 'react';
import type { PageId } from '../lib/types';
import {
  IconHome,
  IconLibrary,
  IconMusic,
  IconSearch,
  IconSettings,
  IconUsers,
} from './Icons';
import { useTogether } from '../store/together';

interface Props {
  page: PageId;
  onNavigate: (p: PageId) => void;
  version?: string;
}

const MAIN: { id: PageId; label: string; icon: ReactNode }[] = [
  { id: 'home', label: '音乐首页', icon: <IconHome size={18} /> },
  { id: 'search', label: '搜索', icon: <IconSearch size={18} /> },
  { id: 'discover', label: '网易云榜单', icon: <IconMusic size={18} /> },
  { id: 'library', label: '我的音乐', icon: <IconLibrary size={18} /> },
  { id: 'together', label: '一起听', icon: <IconUsers size={18} /> },
];

export function Sidebar({ page, onNavigate, version }: Props) {
  const joined = useTogether((s) => s.joined);
  const room = useTogether((s) => s.room);

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

      <div className="sidebar-foot">
        <div>纯音频 · 零视频解码</div>
        <div>v{version ?? '0.1.0'}</div>
      </div>
    </aside>
  );
}
