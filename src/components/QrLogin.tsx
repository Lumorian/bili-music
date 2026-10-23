import { useEffect, useRef, useState } from 'react';
import QRCode from 'qrcode';
import { api } from '../lib/api';
import { IconX } from './Icons';
import { toast } from './Toast';

interface Props {
  open: boolean;
  onClose: () => void;
  onSuccess: () => void;
}

/** B 站扫码登录：86101 未扫描 / 86090 已扫待确认 / 86038 过期 / 0 成功 */
export function QrLogin({ open, onClose, onSuccess }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const keyRef = useRef('');
  const doneRef = useRef(false);
  const [status, setStatus] = useState('');
  const [qrError, setQrError] = useState('');

  // 回调放进 ref：父组件的 onClose / onSuccess 每次渲染都是新函数，
  // 如果把它们写进轮询 effect 的依赖里，轮询定时器会被不停地销毁重建，
  // 结果就是「扫了码也永远轮询不到成功」。
  const onCloseRef = useRef(onClose);
  const onSuccessRef = useRef(onSuccess);
  onCloseRef.current = onClose;
  onSuccessRef.current = onSuccess;

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    doneRef.current = false;
    keyRef.current = '';
    setQrError('');
    setStatus('正在生成二维码…');

    const generate = async () => {
      try {
        const qr = await api.biliQr();
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
        setStatus('请用 B 站手机客户端扫码');
      } catch (e) {
        if (!cancelled) {
          const msg = (e as Error).message;
          setQrError(msg);
          setStatus('二维码生成失败');
        }
      }
    };
    void generate();

    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!open) return;
    let stopped = false;
    const id = window.setInterval(async () => {
      const key = keyRef.current;
      if (!key || doneRef.current || stopped) return;
      try {
        const res = await api.biliQrPoll(key);
        if (stopped) return;
        if (res.code === 0) {
          doneRef.current = true;
          toast('B 站登录成功');
          onSuccessRef.current();
          onCloseRef.current();
        } else if (res.code === 86090) {
          setStatus('已扫码，请在手机上确认');
        } else if (res.code === 86038) {
          setStatus('二维码已过期，关闭后重新打开');
        } else if (res.code === 86101) {
          setStatus('请用 B 站手机客户端扫码');
        } else if (res.code !== 0) {
          setStatus(res.message || '等待扫码…');
        }
      } catch {
        /* 网络抖动忽略，下一轮再试 */
      }
    }, 2000);
    return () => {
      stopped = true;
      window.clearInterval(id);
    };
  }, [open]);

  if (!open) return null;

  return (
    <div className="modal-mask" onMouseDown={onClose}>
      <div className="modal" onMouseDown={(e) => e.stopPropagation()}>
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 4 }}>
          <h3 style={{ flex: 1, textAlign: 'left' }}>扫码登录 B 站</h3>
          <button className="icon-btn" onClick={onClose}>
            <IconX size={17} />
          </button>
        </div>
        <p className="hint" style={{ textAlign: 'left' }}>
          登录后才能读取你的收藏夹和稍后再看。不登录也能听歌。
        </p>
        <div className="qr-box">
          <canvas ref={canvasRef} />
        </div>
        <p className="hint" style={{ marginBottom: 0 }}>{status}</p>
        {qrError && (
          <p className="hint" style={{ marginBottom: 0, color: 'var(--danger)' }}>{qrError}</p>
        )}
      </div>
    </div>
  );
}
