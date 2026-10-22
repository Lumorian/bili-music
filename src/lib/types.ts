export type Source = 'bilibili' | 'netease';

export interface Track {
  source: Source;
  /** bvid 或 歌曲 id */
  id: string;
  title: string;
  artist: string;
  cover: string;
  duration_ms: number;
  /** B 站用来记 cid */
  extra?: string | null;
  album?: string | null;
  subtitle?: string | null;
}

export interface Paged<T> {
  items: T[];
  page: number;
  has_more: boolean;
  total: number | null;
}

export interface StreamInfo {
  stream_url: string;
  url: string;
  mime: string;
  expires_at: number;
}

export interface UserInfo {
  logged_in: boolean;
  name: string;
  avatar: string;
  mid: string;
}

export interface QrPayload {
  key: string;
  content: string;
}

export interface TrackDetail {
  track: Track;
  desc: string;
  pages: [number, string, number][];
}

export interface MemberInfo {
  id: string;
  name: string;
  avatar: string;
  is_host: boolean;
  position_ms: number;
  playing: boolean;
}

export interface PlaybackState {
  index: number;
  position_ms: number;
  playing: boolean;
  updated_at: number;
  by: string;
}

export interface RoomSnapshot {
  code: string;
  name: string;
  members: MemberInfo[];
  queue: Track[];
  state: PlaybackState;
  host_id: string;
  server_time: number;
}

export interface ChatItem {
  from: string;
  name: string;
  text: string;
  at: number;
}

export interface Settings {
  togetherServer: string;
  nickname: string;
  bilibili: { name: string; avatar: string; loggedIn: boolean };
  netease: { name: string; avatar: string; loggedIn: boolean };
}

export type PlayMode = 'order' | 'repeat' | 'single' | 'shuffle';

export type PageId = 'home' | 'search' | 'discover' | 'library' | 'together' | 'settings';
