import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";

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

const SWITCHES = [
  { value: "-novid", label: "跳过开场动画", code: "-novid" },
  { value: "-high", label: "高优先级启动", code: "-high" },
];

function App() {
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [toast, setToast] = useState("");
  useEffect(() => {
    if (toast) {
      const t = setTimeout(() => setToast(""), 2000);
      return () => clearTimeout(t);
    }
  }, [toast]);
  const [loading, setLoading] = useState(false);
  const [server, setServer] = useState("cn");
  const [args, setArgs] = useState<string[]>([]);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [avatarSrc, setAvatarSrc] = useState("");

  const refresh = async () => {
    try {
      const s = await invoke<StatusInfo>("get_status");
      setStatus(s);
      setServer(s.last_server || "cn");
      setArgs(s.extra_args || []);
      try {
        const b64 = await invoke<string>("get_avatar_path");
        setAvatarSrc(b64);
      } catch { setAvatarSrc(""); }
    } catch (e) { setToast(`获取状态失败: ${e}`); }
  };

  useEffect(() => {
    refresh();
    const u1 = listen<string>("status", (e) => setToast(e.payload));
    const win = getCurrentWindow();
    const u2 = win.onCloseRequested(async (e) => { e.preventDefault(); await win.hide(); });
    const u3 = win.listen("tauri://focus", () => { refresh(); });
    return () => { u1.then(f => f()); u2.then(f => f()); u3.then(f => f()); };
  }, []);

  const handleLaunch = async () => {
    setLoading(true);
    setToast("正在启动...");
    try { setToast(await invoke<string>("launch_server", { server })); }
    catch (e) { setToast(`启动失败: ${e}`); }
    setLoading(false);
    openUrl("https://metadota2.com/zh-cn");
    refresh();
  };

  const toggleArg = async (arg: string) => {
    const next = args.includes(arg) ? args.filter(a => a !== arg) : [...args, arg];
    setArgs(next);
    try { await invoke("set_extra_args", { args: next }); }
    catch (e) { setToast(`保存失败: ${e}`); }
  };

  return (
    <div className="container">
      {/* Account */}
      {status && (
        <div className="account-line">
          <div className="account-avatar">
            {avatarSrc ? <img src={avatarSrc} alt="" style={{width:"100%",height:"100%",borderRadius:"7px",objectFit:"cover"}} /> : "D2"}
          </div>
          <div className="account-text">
            <div className="account-name">{status.active_user_name}</div>
            <div className="account-id">{status.active_user_id}</div>
          </div>
        </div>
      )}

      {/* Server Select */}
      <div style={{position:"relative"}}>
        <div className="section-label">服务器</div>
        <div className={`select-wrap ${dropdownOpen ? "open" : ""}`} onClick={() => setDropdownOpen(!dropdownOpen)}>
          <div className="select-trigger">
            {server === "cn" ? "国服" : "全球服"}
            <span className="select-sub">{server === "cn" ? "Perfect World" : "International"}</span>
          </div>
          <span className="select-arrow" />
        </div>
        {dropdownOpen && (
          <div className="dropdown-menu">
            <div className={`dropdown-item ${server === "cn" ? "active" : ""}`} onClick={() => { setServer("cn"); setDropdownOpen(false); }}>
              <span className="dd-title">国服</span>
              <span className="dd-sub">Perfect World</span>
            </div>
            <div className={`dropdown-item ${server === "global" ? "active" : ""}`} onClick={() => { setServer("global"); setDropdownOpen(false); }}>
              <span className="dd-title">全球服</span>
              <span className="dd-sub">International</span>
            </div>
          </div>
        )}
      </div>

      {/* Switches */}
      <div>
        <div className="section-label">启动参数</div>
        <div className="switch-group">
          {SWITCHES.map(sw => (
            <label key={sw.value} className="switch-row">
              <span className="switch-label">
                {sw.label}
                <span className="arg-code">{sw.code}</span>
              </span>
              <span className="toggle">
                <input
                  type="checkbox"
                  checked={args.includes(sw.value)}
                  onChange={() => toggleArg(sw.value)}
                />
                <span className="toggle-track" />
                <span className="toggle-thumb" />
              </span>
            </label>
          ))}
        </div>
      </div>

      {/* Launch Button */}
      <div className="launch-area">
        <button className="launch-btn" onClick={handleLaunch} disabled={loading || !status}>
          {loading ? "启动中..." : "启动 DOTA 2"}
        </button>
      </div>

      {/* Toast */}
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

export default App;
