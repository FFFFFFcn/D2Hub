import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface StatusInfo {
  steam_path: string;
  active_user_id: string;
  active_user_name: string;
  stored_launch_options: string;
  last_server: string;
  cleaned: boolean;
  extra_args: string[];
  steam_running: boolean;
}

const EXTRA_OPTIONS = [
  { value: "-novid", label: "跳过开场动画 (-novid)" },
  { value: "-console", label: "开发者控制台 (-console)" },
  { value: "-language schinese", label: "简体中文 (-language schinese)" },
  { value: "-high", label: "高 CPU 优先级 (-high)" },
];

function App() {
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(false);
  const [extraArgs, setExtraArgs] = useState<string[]>([]);

  const refreshStatus = async () => {
    try {
      const s = await invoke<StatusInfo>("get_status");
      setStatus(s);
      setExtraArgs(s.extra_args);
    } catch (e) {
      setMessage(`获取状态失败: ${e}`);
    }
  };

  useEffect(() => {
    refreshStatus();
    const unlisten = listen<string>("status", (event) => {
      setMessage(event.payload);
    });

    // 拦截关闭事件：关设置窗口 = 隐藏，不退出程序
    const win = getCurrentWindow();
    const unlistenClose = win.onCloseRequested(async (e) => {
      e.preventDefault();
      await win.hide();
    });

    return () => {
      unlisten.then((f) => f());
      unlistenClose.then((f) => f());
    };
  }, []);

  const handleLaunch = async (server: string) => {
    setLoading(true);
    setMessage(`正在启动${server === "cn" ? "国服" : "全球服"}…`);
    try {
      const result = await invoke<string>("launch_server", { server });
      setMessage(result);
    } catch (e) {
      setMessage(`启动失败: ${e}`);
    }
    setLoading(false);
    refreshStatus();
  };

  const handleCleanup = async () => {
    setLoading(true);
    setMessage("正在清理启动选项…");
    try {
      const result = await invoke<string>("cleanup_options");
      setMessage(result);
    } catch (e) {
      setMessage(`清理失败: ${e}`);
    }
    setLoading(false);
    refreshStatus();
  };

  const toggleExtraArg = async (arg: string) => {
    const next = extraArgs.includes(arg)
      ? extraArgs.filter((a) => a !== arg)
      : [...extraArgs, arg];
    setExtraArgs(next);
    try {
      await invoke("set_extra_args", { args: next });
    } catch (e) {
      setMessage(`保存参数失败: ${e}`);
    }
  };

  const serverLabel = status?.stored_launch_options?.includes("-perfectworld")
    ? "国服（Steam 已存 -perfectworld）"
    : status?.stored_launch_options
      ? `自定义: ${status.stored_launch_options}`
      : "全球服（Steam 启动选项为空）";

  return (
    <div className="container">
      <h1>🎮 Dota2 切换器 - 设置</h1>

      {message && (
        <div className="message">
          {message}
        </div>
      )}

      <div className="section">
        <h2>状态</h2>
        {status ? (
          <table className="status-table">
            <tbody>
              <tr>
                <td>Steam 路径</td>
                <td><code>{status.steam_path}</code></td>
              </tr>
              <tr>
                <td>活跃账号</td>
                <td>{status.active_user_name} <code>({status.active_user_id})</code></td>
              </tr>
              <tr>
                <td>Steam 启动选项</td>
                <td><strong>{serverLabel}</strong></td>
              </tr>
              <tr>
                <td>上次启动</td>
                <td>{status.last_server === "cn" ? "🇨🇳 国服" : "🌍 全球服"}</td>
              </tr>
              <tr>
                <td>已清理</td>
                <td>{status.cleaned ? "✅ 是" : "❌ 否"}</td>
              </tr>
              <tr>
                <td>Steam 状态</td>
                <td>{status.steam_running ? "🟢 运行中" : "⚪ 未运行"}</td>
              </tr>
            </tbody>
          </table>
        ) : (
          <p>加载中…</p>
        )}
      </div>

      <div className="section">
        <h2>快速启动</h2>
        <div className="btn-group">
          <button
            className="btn btn-cn"
            onClick={() => handleLaunch("cn")}
            disabled={loading}
          >
            🇨🇳 启动国服
          </button>
          <button
            className="btn btn-global"
            onClick={() => handleLaunch("global")}
            disabled={loading}
          >
            🌍 启动全球服
          </button>
        </div>
      </div>

      <div className="section">
        <h2>额外启动参数</h2>
        {EXTRA_OPTIONS.map((opt) => (
          <label key={opt.value} className="checkbox-label">
            <input
              type="checkbox"
              checked={extraArgs.includes(opt.value)}
              onChange={() => toggleExtraArg(opt.value)}
            />
            {opt.label}
          </label>
        ))}
      </div>

      <div className="section">
        <h2>维护</h2>
        <button className="btn btn-warn" onClick={handleCleanup} disabled={loading}>
          🧹 重新清理 Steam 启动选项
        </button>
        <p className="hint">
          如果手动在 Steam 里重新添加了 -perfectworld，点此清理。
        </p>
      </div>
    </div>
  );
}

export default App;
