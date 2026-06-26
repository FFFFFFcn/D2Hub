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
  { value: "-novid", label: "novid" },
  { value: "-console", label: "console" },
  { value: "-language schinese", label: "schinese" },
  { value: "-high", label: "high" },
];

function LoadingSkeleton() {
  return (
    <>
      <div className="status-grid">
        {Array.from({ length: 6 }).map((_, i) => (
          <div className="status-card" key={i}>
            <div className="skeleton-block short" />
            <div className="skeleton-block med" />
          </div>
        ))}
      </div>
      <div className="launch-group">
        <div className="launch-btn" style={{ opacity: 0.3 }}>
          <span className="icon">&#8203;</span>
          <span className="sub">loading</span>
        </div>
        <div className="launch-btn" style={{ opacity: 0.3 }}>
          <span className="icon">&#8203;</span>
          <span className="sub">loading</span>
        </div>
      </div>
    </>
  );
}

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

  const storedIsCN = status?.stored_launch_options?.includes("-perfectworld");
  const lastIsCN = status?.last_server === "cn";
  const storedLabel = status?.stored_launch_options
    ? storedIsCN
      ? "CN "
      : status.stored_launch_options.length > 0
        ? status.stored_launch_options
        : ""
    : "";

  return (
    <div className="container">
      <h1>Dota2 Switcher</h1>

      {message && <div className="message">{message}</div>}

      {!status ? (
        <LoadingSkeleton />
      ) : (
        <>
          {/* ─── Status Grid ─── */}
          <h2>Status</h2>
          <div className="status-grid">
            <div className="status-card span-2">
              <span className="label">Account</span>
              <span className="value">
                {status.active_user_name}{" "}
                <code>{status.active_user_id}</code>
              </span>
            </div>
            <div className="status-card">
              <span className="label">Steam</span>
              <span className="value">
                <span className="indicator">
                  <span
                    className={`indicator-dot ${status.steam_running ? "online" : "offline"}`}
                  />
                  {status.steam_running ? "Online" : "Offline"}
                </span>
              </span>
            </div>
            <div className="status-card">
              <span className="label">Last Launch</span>
              <span className="value">
                <span className="indicator">
                  <span
                    className={`indicator-dot ${lastIsCN ? "cn" : "global"}`}
                  />
                  {lastIsCN ? "CN " : "Global"}
                </span>
              </span>
            </div>
            <div className="status-card">
              <span className="label">Stored Option</span>
              <span className="value">
                {storedLabel || (
                  <span style={{ color: "var(--text-dim)" }}>empty</span>
                )}
              </span>
            </div>
            <div className="status-card">
              <span className="label">Cleaned</span>
              <span className="value">{status.cleaned ? "Done" : "Pending"}</span>
            </div>
          </div>

          {/* ─── Launch ─── */}
          <h2>Launch</h2>
          <div className="launch-group">
            <button
              className="launch-btn cn"
              onClick={() => handleLaunch("cn")}
              disabled={loading}
            >
              <span className="icon"></span>
              Perfect World
              <span className="sub">dota2 &mdash; cn server</span>
            </button>
            <button
              className="launch-btn global"
              onClick={() => handleLaunch("global")}
              disabled={loading}
            >
              <span className="icon"></span>
              Global
              <span className="sub">dota2 &mdash; international</span>
            </button>
          </div>

          {/* ─── Extra Args ─── */}
          <h2>Extra Args</h2>
          <div className="args-section">
            {EXTRA_OPTIONS.map((opt) => (
              <label
                key={opt.value}
                className={`arg-chip ${extraArgs.includes(opt.value) ? "checked" : ""}`}
                onClick={() => toggleExtraArg(opt.value)}
              >
                <span className="chip-dot" />
                {opt.label}
              </label>
            ))}
          </div>

          {/* ─── Maintenance ─── */}
          <h2>Maintenance</h2>
          <div className="maintenance-card">
            <div className="maintenance-info">
              <p className="title">Reset Steam Launch Options</p>
              <p className="desc">
                Clear the stored -perfectworld flag from Steam's config file.
                Required once after first setup.
              </p>
            </div>
            <button
              className="btn-sm warn"
              onClick={handleCleanup}
              disabled={loading}
            >
              Clean
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default App;
