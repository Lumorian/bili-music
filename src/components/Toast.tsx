import { create } from 'zustand';

interface ToastItem {
  id: number;
  text: string;
  kind: 'info' | 'error';
}

interface ToastState {
  items: ToastItem[];
  push: (text: string, kind?: 'info' | 'error') => void;
  remove: (id: number) => void;
}

const useToast = create<ToastState>((set, get) => ({
  items: [],
  push: (text, kind = 'info') => {
    const id = Date.now() + Math.random();
    set({ items: [...get().items, { id, text, kind }] });
    setTimeout(() => get().remove(id), 3200);
  },
  remove: (id) => set({ items: get().items.filter((t) => t.id !== id) }),
}));

/** 全局一次性调用：toast('需要登录') */
export function toast(text: string, kind: 'info' | 'error' = 'info') {
  useToast.getState().push(text, kind);
}

export function ToastHost() {
  const items = useToast((s) => s.items);
  if (!items.length) return null;
  return (
    <div className="toast-wrap">
      {items.map((t) => (
        <div key={t.id} className={`toast ${t.kind === 'error' ? 'error' : ''}`}>
          {t.text}
        </div>
      ))}
    </div>
  );
}
