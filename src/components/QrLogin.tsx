import { useEffect, useRef, useState } from 'react';
import QRCode from 'qrcode';
import { api } from '../lib/api';
import { IconX } from './Icons';
import { toast } from './Toast';

type Platform = 'bilibili' | 'netease';

interface Props {
  platform: Platform;
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

const TEXT: Record<Platform, { title: string; hint: string }> = {
  bilibili: { title: '扫码登录 B 站', hint: '用 B 站手机客户端扫码，登录后可同步收藏夹与稍后再看' },
  netease: { title: '扫码登录网易云', hint: '用网易云音乐 App 扫码，登录后才能播放会员 / 付费歌曲' },
};

export function QrLogin({ platform, open, onClose, onSuccess }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const keyRef = useRef('');
  const doneRef = useRef(false);
  const [status, setStatus] = useState('');
  const [tick, setTick] = useState(0);

  // 每次打开都重新生成二维码
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    doneRef.current = false;
    keyRef.current = '';
    setStatus('正在生成二维码…');

    const generate = async () => {
      try {
        const qr = platform === 'bilibili' ? await api.biliQr() : await api.neQr();
        if (cancelled) return;
        keyRef.current = qr.key;
        const canvas = canvasRef.current;
        if (canvas) {
          await QRCode.toCanvas(canvas, qr.content, {
            width: 184,
            margin: 1,
            color: { dark: '#14151a', light: '#ffffff' },
          });
        }
        setStatus('请用手机 App 扫码');
      } catch (e) {
        if (!cancelled) setStatus((e as Error).message);
      }
    };
    void generate();

    return () => {
      cancelled = true;
    };
  }, [open, platform]);

  // 轮询扫码状态（不依赖渲染，靠 ref 拿 key）
  useEffect(() => {
    if (!open) return;
    const id = window.setInterval(async () => {
      const key = keyRef.current;
      if (!key || doneRef.current) return;
      try {
        const res = platform === 'bilibili' ? await api.biliQrPoll(key) : await api.neQrPoll(key);
        const ok = platform === 'bilibili' ? res.code === 0 : res.code === 803;
        const scanned = platform === 'bilibili' ? res.code === 86090 : res.code === 802;
        const expired = platform === 'bilibili' ? res.code === 86038 : res.code === 800;

        if (ok) {
          doneRef.current = true;
          toast(platform === 'bilibili' ? 'B 站登录成功' : '网易云登录成功');
          onSuccess();
          onClose();
        } else if (scanned) {
          setStatus('已扫码，请在手机上确认');
        } else if (expired) {
          setStatus('二维码已过期，关闭后重新打开');
        }
      } catch {
        /* 网络抖动忽略，下一轮再试 */
      }
    }, 2000);
    return () => window.clearInterval(id);
  }, [open, platform, onClose, onSuccess, tick]);

  if (!open) return null;

  return (
    <div className="modal-mask" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 4 }}>
          <h3 style={{ flex: 1, textAlign: 'left' }}>{TEXT[platform].title}</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconX size={17} />
          </button>
        </div>
        <p className="hint" style={{ textAlign: 'left' }}>{TEXT[platform].hint}</p>
        <div className="qr-box">
          <canvas ref={canvasRef} />
        </div>
        <p className="hint" style={{ marginBottom: 0 }}>{status}</p>
      </div>
    </div>
  );
}
