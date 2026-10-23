import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { version } from "../package.json";
import { importDllPaths, listenForFileDrops } from "./fileDrop";
import { toggleModSelection, toggleVisibleSelection } from "./selection";
import {
  Box,
  LayoutGrid,
  Settings2,
  FileText,
  Plus,
  Search,
  ArrowUpRight,
  Play,
  FolderOpen,
  Trash2,
  PackageOpen,
  ShieldCheck,
  RefreshCw,
  X,
  ChevronRight,
  Download,
  Power,
  CircleHelp,
} from "lucide-react";
import { command, desktop, getSnapshot, type Snapshot, type Mod } from "./api";
import "./App.css";
type Page = "library" | "setup" | "logs" | "guide";
export default function App() {
  const [data, setData] = useState<Snapshot>();
  const [page, setPage] = useState<Page>("library");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const [busy, setBusy] = useState(false);
  const working = useRef(false);
  const dropImport = useRef<(paths: string[]) => void>(() => {});
  const [dragging, setDragging] = useState(false);
  const [notice, setNotice] = useState<{ text: string; error: boolean }>();
  const [remove, setRemove] = useState<Mod[]>();
  const [selection, setSelection] = useState<Set<string>>(() => new Set());
  const selectionAnchor = useRef<string | undefined>(undefined);
  const selectAllBox = useRef<HTMLInputElement>(null);
  const refresh = async () => {
    try {
      setData(await getSnapshot());
    } catch (e) {
      setNotice({ text: String(e), error: true });
    }
  };
  useEffect(() => {
    void refresh();
    const t = setInterval(() => void refresh(), 5000);
    return () => clearInterval(t);
  }, []);
  const action = async (
    name: string,
    args?: Record<string, unknown>,
    message = "変更を保存しました。",
  ) => {
    if (working.current) return false;
    working.current = true;
    setBusy(true);
    try {
      await command(name, args);
      await refresh();
      setNotice({ text: message, error: false });
      return true;
    } catch (e) {
      setNotice({ text: String(e), error: true });
      return false;
    } finally {
      working.current = false;
      setBusy(false);
    }
  };
  const importPaths = async (paths: string[]) => {
    if (!paths.length) return;
    if (working.current || data?.running || remove) {
      setNotice({
        text: data?.running
          ? "Minecraft を終了してから DLL を追加してください。"
          : remove
            ? "削除の確認画面を閉じてから DLL を追加してください。"
            : "現在の処理が完了してから DLL を追加してください。",
        error: true,
      });
      return;
    }
    working.current = true;
    setBusy(true);
    try {
      const { count, updated, unchanged, errors } = await importDllPaths(
        paths,
        (path) => command("import_mod", { path }),
      );
      if (count + updated + unchanged > 0) {
        setPage("library");
        setQuery("");
        setFilter("all");
      }
      await refresh();
      setNotice({
        text: `${count} 件追加・${updated} 件更新・${unchanged} 件変更なし。${errors.join(" / ")}`,
        error: errors.length > 0,
      });
    } catch (e) {
      setNotice({ text: String(e), error: true });
    } finally {
      working.current = false;
      setBusy(false);
    }
  };
  useEffect(() => {
    dropImport.current = (paths) => {
      void importPaths(paths);
    };
  });
  useEffect(() => {
    if (!desktop) return;
    return listenForFileDrops(
      (handler) => getCurrentWebview().onDragDropEvent(handler),
      setDragging,
      (paths) => dropImport.current(paths),
      (error) =>
        setNotice({
          text: `ドラッグ＆ドロップを準備できませんでした。「DLL を追加」から選択できます。${String(error)}`,
          error: true,
        }),
    );
  }, []);
  const add = async () => {
    if (!desktop) {
      setNotice({
        text: "DLL の追加は Windows アプリで利用できます。",
        error: true,
      });
      return;
    }
    if (working.current) return;
    working.current = true;
    setBusy(true);
    let files: string | string[] | null = null;
    try {
      files = await open({
        multiple: true,
        filters: [{ name: "Windows DLL", extensions: ["dll"] }],
      });
    } catch (e) {
      setNotice({ text: String(e), error: true });
    } finally {
      working.current = false;
      setBusy(false);
    }
    if (files) await importPaths(Array.isArray(files) ? files : [files]);
  };
  const choose = async () => {
    try {
      const p = await open({
        directory: true,
        title: "Minecraft の Content フォルダを選択",
      });
      if (typeof p === "string")
        await action(
          "set_game_path",
          { path: p },
          "Minecraft の場所を保存しました。",
        );
    } catch (e) {
      setNotice({ text: String(e), error: true });
    }
  };
  const mods = data?.library.mods ?? [];
  const enabled = mods.filter((m) => m.enabled).length;
  const locked = busy || !!data?.running;
  const shown = mods.filter(
    (m) =>
      m.name.toLowerCase().includes(query.toLowerCase()) &&
      (filter === "all" || (filter === "enabled" ? m.enabled : !m.enabled)),
  );
  const visibleIds = shown.map((mod) => mod.id);
  const selectedMods = mods.filter((mod) => selection.has(mod.id));
  const visibleSelected = shown.filter((mod) => selection.has(mod.id)).length;
  const allVisibleSelected =
    shown.length > 0 && visibleSelected === shown.length;
  useEffect(() => {
    if (selectAllBox.current)
      selectAllBox.current.indeterminate =
        visibleSelected > 0 && !allVisibleSelected;
  }, [visibleSelected, allVisibleSelected, page]);
  const selectMod = (id: string, range: boolean) => {
    setSelection((current) =>
      toggleModSelection(
        current,
        id,
        visibleIds,
        selectionAnchor.current,
        range,
      ),
    );
    if (
      !range ||
      !selectionAnchor.current ||
      !visibleIds.includes(selectionAnchor.current)
    )
      selectionAnchor.current = id;
  };
  const nav = [
    { id: "library" as const, label: "Mod ライブラリ", icon: LayoutGrid },
    { id: "setup" as const, label: "ゲームとローダー", icon: Settings2 },
    { id: "logs" as const, label: "読み込みログ", icon: FileText },
  ];
  return (
    <div
      className="app-shell"
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault();
        if (!desktop)
          setNotice({
            text: "ドラッグ＆ドロップは Windows アプリで利用できます。",
            error: true,
          });
      }}
    >
      {dragging && (
        <div
          className={`file-drop-overlay ${locked || remove ? "blocked" : ""}`}
          role="status"
          aria-live="polite"
        >
          <div className="file-drop-card">
            <Download size={40} strokeWidth={1.5} />
            <h2>
              {data?.running
                ? "Minecraft を終了してから追加してください"
                : busy
                  ? "現在の処理が完了するまでお待ちください"
                  : remove
                    ? "削除の確認画面を閉じてください"
                    : "ここに DLL をドロップ"}
            </h2>
            <p>
              {locked || remove
                ? "この状態ではファイルを追加しません。"
                : "複数の DLL をまとめて追加できます。追加後は無効で登録されます。"}
            </p>
          </div>
        </div>
      )}
      <aside className="sidebar">
        <button className="brand" onClick={() => setPage("library")}>
          <span className="brand-mark">
            <Box size={25} />
          </span>
          <span>
            <strong>
              PEX<span className="brand-light">server</span>
            </strong>
            <small>MOD STUDIO</small>
          </span>
        </button>
        <div className="workspace-label">
          WORKSPACE <span>01</span>
        </div>
        <nav aria-label="メインメニュー">
          {nav.map((n) => (
            <button
              key={n.id}
              className={`nav-item ${page === n.id ? "selected" : ""}`}
              onClick={() => setPage(n.id)}
            >
              <n.icon size={18} />
              {n.label}
              {n.id === "library" && (
                <span className="nav-count">{mods.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="edition">
            <span className="edition-icon">
              <Box size={20} />
            </span>
            <div>
              <strong>Minecraft for Windows</strong>
              <small>BEDROCK · GDK / x64</small>
            </div>
          </div>
          <button
            className={`nav-item ${page === "guide" ? "selected" : ""}`}
            onClick={() => setPage("guide")}
          >
            <CircleHelp size={18} />
            使い方と互換性
            <ArrowUpRight size={15} />
          </button>
          <div className="sidebar-footer">
            <span className="status-dot" />
            LOCAL WORKSPACE <span>v{version}</span>
          </div>
        </div>
      </aside>
      <main>
        <header className="topbar">
          <span>
            WORKSPACE <ChevronRight size={13} />
            {page === "library"
              ? "Mod ライブラリ"
              : page === "setup"
                ? "ゲームとローダー"
                : page === "logs"
                  ? "読み込みログ"
                  : "使い方と互換性"}
          </span>
          <span className="topbar-right">
            <span className={`status-dot ${data?.running ? "live" : ""}`} />
            {data?.running ? "Minecraft 起動中" : "Minecraft 停止中"}
            <button
              className="icon-button"
              aria-label="状態を更新"
              disabled={busy}
              onClick={() => void refresh()}
            >
              <RefreshCw size={16} />
            </button>
          </span>
        </header>
        <div className="content">
          <div className="page-heading">
            <div>
              <div className="eyebrow">MINECRAFT, YOUR WAY.</div>
              <h1>
                {page === "library" ? (
                  <>
                    遊び方を、<span>自分らしく。</span>
                  </>
                ) : page === "setup" ? (
                  "いつもの起動に、Mod を。"
                ) : page === "logs" ? (
                  "読み込みを、確かめる。"
                ) : (
                  "はじめての Mod Studio。"
                )}
              </h1>
              <p>
                {page === "library"
                  ? "お気に入りの DLL Mod を、ひとつの場所で管理。"
                  : page === "setup"
                    ? "ゲームの場所と、自動読み込みの設定を管理します。"
                    : page === "logs"
                      ? "ゲーム内のローダーから届く、実際の読み込み記録です。"
                      : "DLL の追加から、Minecraft の起動まで。"}
              </p>
            </div>
            <div className="heading-stamp">
              <Box size={25} />
              <span>
                BUILT FOR
                <br />
                <b>BEDROCK.</b>
              </span>
            </div>
          </div>
          {!desktop && (
            <div className="inline-note">
              ブラウザプレビューです。ゲームの検出・DLL の管理は Windows
              アプリで利用できます。
            </div>
          )}
          {notice && (
            <div
              className={`notice ${notice.error ? "error" : ""}`}
              role={notice.error ? "alert" : "status"}
            >
              <span>{notice.text}</span>
              <button
                className="icon-button"
                aria-label="通知を閉じる"
                onClick={() => setNotice(undefined)}
              >
                <X size={16} />
              </button>
            </div>
          )}
          {data?.running && (
            <div className="inline-note">
              Minecraft が起動中です。Mod の変更はゲーム終了後に行えます。
            </div>
          )}
          {page === "library" && (
            <>
              <section className="launch-card">
                <div className="launch-art" aria-hidden="true">
                  <div className="voxel v1" />
                  <div className="voxel v2" />
                  <div className="voxel v3" />
                  <div className="voxel v4" />
                  <span className="art-cross">+</span>
                </div>
                <div className="launch-copy">
                  <div className="eyebrow">YOUR NEXT ADVENTURE</div>
                  <h2>Minecraft for Windows</h2>
                  <p>
                    {data?.detected
                      ? `GDK ${data.gameVersion} · x64`
                      : "ゲームの場所を設定してください"}
                  </p>
                  <div className="pills">
                    <span className={`pill ${data?.healthy ? "good" : ""}`}>
                      <span className="status-dot" />
                      {data?.healthy
                        ? "自動読み込み ON"
                        : data?.installed
                          ? "ローダー要確認"
                          : "ローダー未導入"}
                    </span>
                    <span className="pill subtle">有効な Mod {enabled} 件</span>
                  </div>
                </div>
                <div className="launch-action">
                  <button
                    className="primary launch-button"
                    disabled={busy || !data?.detected || data?.running}
                    onClick={() =>
                      void action(
                        "launch_game",
                        undefined,
                        "起動を要求しました。ゲームの起動状態を確認しています。",
                      )
                    }
                  >
                    <Play size={16} fill="currentColor" />
                    Minecraft を起動
                    <ArrowUpRight size={17} />
                  </button>
                  <span>
                    {data?.healthy
                      ? "いつもの起動でも、自動で読み込み。"
                      : "ゲームとローダーから初期設定できます。"}
                  </span>
                </div>
              </section>
              <section className="stats" aria-label="ライブラリの統計">
                <div>
                  <span>ライブラリ</span>
                  <strong>
                    {String(mods.length).padStart(2, "0")}
                    <small>MODS</small>
                  </strong>
                  <PackageOpen size={21} />
                </div>
                <div>
                  <span>有効な Mod</span>
                  <strong>
                    {String(enabled).padStart(2, "0")}
                    <small>ENABLED</small>
                  </strong>
                  <Power size={21} />
                </div>
                <div>
                  <span>次回起動に反映</span>
                  <strong className="stat-text">
                    {data?.healthy ? "準備完了" : "セットアップ"}
                    <small>{data?.healthy ? "READY" : "REQUIRED"}</small>
                  </strong>
                  <ShieldCheck size={21} />
                </div>
              </section>
              <section className="library-section">
                <div className="section-heading">
                  <div>
                    <div className="eyebrow">YOUR COLLECTION</div>
                    <h2>
                      Mod ライブラリ <span>{mods.length}</span>
                    </h2>
                  </div>
                  <button
                    className="primary"
                    disabled={locked}
                    onClick={() => void add()}
                  >
                    <Plus size={18} />
                    DLL を追加
                  </button>
                </div>
                <div className="library-toolbar">
                  <div
                    className="tabs"
                    role="group"
                    aria-label="Mod の表示条件"
                  >
                    {[
                      ["all", "すべて", mods.length],
                      ["enabled", "有効", enabled],
                      ["disabled", "無効", mods.length - enabled],
                    ].map(([id, label, count]) => (
                      <button
                        key={id}
                        className={filter === id ? "active" : ""}
                        onClick={() => setFilter(String(id))}
                      >
                        {label}
                        <span>{count}</span>
                      </button>
                    ))}
                  </div>
                  <label className="search">
                    <Search size={16} />
                    <input
                      aria-label="Mod を検索"
                      placeholder="Mod を検索..."
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                    />
                  </label>
                </div>
                <p className="drop-hint">
                  DLL
                  をこのウィンドウにドラッグ＆ドロップして追加できます（複数選択可）。
                </p>
                {mods.length > 0 && (
                  <div
                    className={`selection-toolbar ${selectedMods.length ? "has-selection" : ""}`}
                    aria-label="Mod の一括操作"
                  >
                    <label className="select-all">
                      <input
                        ref={selectAllBox}
                        type="checkbox"
                        checked={allVisibleSelected}
                        disabled={!shown.length || busy}
                        onChange={() =>
                          setSelection((current) =>
                            toggleVisibleSelection(current, visibleIds),
                          )
                        }
                      />
                      表示中を全選択
                    </label>
                    <span
                      className="selection-count"
                      role="status"
                      aria-live="polite"
                    >
                      {selectedMods.length} 件選択
                      {selectedMods.length > visibleSelected
                        ? `（うち非表示 ${selectedMods.length - visibleSelected} 件）`
                        : ""}
                    </span>
                    <div className="bulk-actions">
                      <button
                        className="secondary"
                        disabled={locked || !selectedMods.length}
                        onClick={() =>
                          void action(
                            "set_mods_enabled",
                            {
                              ids: selectedMods.map((mod) => mod.id),
                              enabled: true,
                            },
                            `${selectedMods.length} 件を有効にしました。次回起動に反映されます。`,
                          )
                        }
                      >
                        <Power size={14} />
                        有効化
                      </button>
                      <button
                        className="secondary"
                        disabled={locked || !selectedMods.length}
                        onClick={() =>
                          void action(
                            "set_mods_enabled",
                            {
                              ids: selectedMods.map((mod) => mod.id),
                              enabled: false,
                            },
                            `${selectedMods.length} 件を無効にしました。次回起動に反映されます。`,
                          )
                        }
                      >
                        無効化
                      </button>
                      <button
                        className="secondary danger"
                        disabled={locked || !selectedMods.length}
                        onClick={() => setRemove(selectedMods)}
                      >
                        <Trash2 size={14} />
                        削除
                      </button>
                      <button
                        className="text-button"
                        disabled={!selectedMods.length || busy}
                        onClick={() => {
                          setSelection(new Set());
                          selectionAnchor.current = undefined;
                        }}
                      >
                        選択解除
                      </button>
                    </div>
                  </div>
                )}
                {mods.length > 0 && (
                  <p className="selection-help">
                    行をクリックして選択 · Shift＋クリックで範囲選択 · Space
                    キーでチェック切り替え
                  </p>
                )}
                {shown.length ? (
                  <div className="mod-list">
                    {shown.map((m) => (
                      <article
                        className={`mod-row ${selection.has(m.id) ? "is-selected" : ""}`}
                        key={m.id}
                        onClick={(event) => {
                          if (
                            !(event.target as HTMLElement).closest(
                              "button,input,label,a",
                            ) &&
                            !busy
                          )
                            selectMod(m.id, event.shiftKey);
                        }}
                      >
                        <label className="mod-checkbox">
                          <input
                            type="checkbox"
                            aria-label={`${m.name} を選択`}
                            checked={selection.has(m.id)}
                            disabled={busy}
                            onChange={(event) =>
                              selectMod(
                                m.id,
                                (event.nativeEvent as MouseEvent).shiftKey ===
                                  true,
                              )
                            }
                          />
                        </label>
                        <span className={`mod-icon ${m.enabled ? "on" : ""}`}>
                          <Box size={24} />
                        </span>
                        <div className="mod-info">
                          <h3>
                            {m.name}
                            <span
                              className={`mod-badge ${m.enabled ? "on" : ""}`}
                            >
                              {m.enabled ? "有効" : "無効"}
                            </span>
                          </h3>
                          <p>
                            {m.filename} · {(m.size / 1024 / 1024).toFixed(2)}{" "}
                            MB · x64 DLL
                          </p>
                          <code title={m.sha256}>
                            SHA-256 {m.sha256.slice(0, 16)}…
                          </code>
                        </div>
                        <button
                          className="switch"
                          role="switch"
                          aria-label={`${m.name} を有効にする`}
                          aria-checked={m.enabled}
                          disabled={locked}
                          onClick={() =>
                            void action(
                              "set_enabled",
                              { id: m.id, enabled: !m.enabled },
                              `${m.name} を${m.enabled ? "無効" : "有効"}にしました。次回起動に反映されます。`,
                            )
                          }
                        >
                          <span />
                        </button>
                        <button
                          className="icon-button delete"
                          aria-label={`${m.name} を削除`}
                          disabled={locked}
                          onClick={() => setRemove([m])}
                        >
                          <Trash2 size={17} />
                        </button>
                      </article>
                    ))}
                  </div>
                ) : (
                  <div className="empty-state">
                    <div className="empty-icon">
                      <PackageOpen size={35} strokeWidth={1.25} />
                      <span>+</span>
                    </div>
                    <h3>
                      {mods.length
                        ? "該当する Mod がありません"
                        : "最初の Mod を、ここに。"}
                    </h3>
                    <p>
                      {mods.length
                        ? "検索やフィルターを変更してください。"
                        : "お持ちの x64 DLL をこのウィンドウにドロップしてください。"}
                      <br />
                      {!mods.length &&
                        "追加した Mod は、確認してから有効にできます。"}
                    </p>
                    {!mods.length && (
                      <button
                        className="secondary"
                        disabled={locked}
                        onClick={() => void add()}
                      >
                        <Plus size={17} />
                        DLL ファイルを選択
                      </button>
                    )}
                    <small>DRAG & DROP · .DLL FILES · x64</small>
                  </div>
                )}
              </section>
              <div className="bottom-note">
                <ShieldCheck size={17} />
                <p>
                  設定は、次の冒険から。
                  <span>
                    有効・無効の変更は Minecraft の次回起動時に反映されます。
                  </span>
                </p>
                <button
                  className="text-button"
                  onClick={() => setPage("guide")}
                >
                  使い方を見る
                  <ArrowUpRight size={15} />
                </button>
              </div>
            </>
          )}
          {page === "setup" && (
            <div className="settings-grid">
              <section className="panel">
                <div className="eyebrow">01 / GAME DIRECTORY</div>
                <h2>Minecraft の場所</h2>
                <p>XboxGames のゲームフォルダ、または Content を選択します。</p>
                <code className="path">{data?.library.gamePath}</code>
                <div className="setting-status">
                  <span className="status-dot" />
                  {data?.detected
                    ? `Minecraft GDK ${data.gameVersion} を検出`
                    : "ゲームが見つかりません"}
                </div>
                <button
                  className="secondary"
                  disabled={locked || data?.installed}
                  onClick={() => void choose()}
                >
                  <FolderOpen size={17} />
                  フォルダを選択
                </button>
              </section>
              <section className="panel">
                <div className="eyebrow">02 / AUTOMATIC LOADING</div>
                <h2>自動読み込み</h2>
                <p>
                  一度導入すると、Xbox アプリから起動した場合も有効な DLL Mod
                  を読み込みます。
                </p>
                <div className="loader-detail">
                  <ShieldCheck size={24} />
                  <div>
                    <strong>
                      {data?.healthy
                        ? "導入済み・ファイル確認 OK"
                        : data?.installed
                          ? "導入ファイルを確認してください"
                          : "ローダー未導入"}
                    </strong>
                    <small>{data?.loaderVersion}</small>
                  </div>
                </div>
                <div className="button-row">
                  <button
                    className="primary"
                    disabled={locked || !data?.detected}
                    onClick={() =>
                      void action(
                        "install_loader",
                        undefined,
                        "自動読み込みローダーを導入しました。",
                      )
                    }
                  >
                    <Download size={17} />
                    {data?.installed ? "構成を確認・同期" : "ローダーを導入"}
                  </button>
                  <button
                    className="secondary danger"
                    disabled={locked || !data?.installed}
                    onClick={() =>
                      void action(
                        "uninstall_loader",
                        undefined,
                        "自動読み込みを解除しました。Mod ライブラリは保持されます。",
                      )
                    }
                  >
                    導入解除
                  </button>
                </div>
              </section>
              <section className="panel full">
                <div className="eyebrow">LOCAL STORAGE</div>
                <h2>ファイルとライブラリ</h2>
                <p>
                  DLL はローカルに保存します。元の DLL
                  ファイルはそのまま残ります。
                </p>
                <code className="path">
                  {data?.dataPath || "Windows アプリで表示"}
                </code>
                <div className="button-row">
                  <button
                    className="secondary"
                    onClick={() =>
                      void action(
                        "open_folder",
                        { kind: "library" },
                        "ライブラリを開きました。",
                      )
                    }
                  >
                    <FolderOpen size={17} />
                    ライブラリを開く
                  </button>
                  <button
                    className="secondary"
                    disabled={!data?.detected}
                    onClick={() =>
                      void action(
                        "open_folder",
                        { kind: "game" },
                        "ゲームフォルダを開きました。",
                      )
                    }
                  >
                    <FolderOpen size={17} />
                    ゲームフォルダを開く
                  </button>
                </div>
              </section>
            </div>
          )}
          {page === "logs" && (
            <section className="panel">
              <div className="section-heading">
                <div>
                  <div className="eyebrow">LOADER OUTPUT</div>
                  <h2>ゲーム側の読み込み記録</h2>
                </div>
                <button className="secondary" onClick={() => void refresh()}>
                  <RefreshCw size={16} />
                  更新
                </button>
              </div>
              <p>
                LOADED は DLL の読み込み成功です。Mod
                の機能とバージョン互換性は、ゲーム内でも確認してください。
              </p>
              <pre className="log-output">
                {data?.logs ||
                  "まだ記録がありません。ローダーを導入して Minecraft を起動すると、ここに結果が表示されます。"}
              </pre>
              <small>最新 64 KB を表示 · 5 秒ごとに更新</small>
            </section>
          )}
          {page === "guide" && (
            <section className="panel guide">
              <div className="eyebrow">QUICK START</div>
              <h2>3 ステップで、自分の Minecraft に。</h2>
              {[
                [
                  "01",
                  "ゲームとローダーを設定",
                  "「ゲームとローダー」で検出された場所を確認し、ローダーを導入します。既存の winhttp.dll がある場合は、衝突するため導入を停止します。",
                ],
                [
                  "02",
                  "DLL を追加して、有効にする",
                  "x64 DLL をウィンドウにドラッグ＆ドロップするか、「DLL を追加」から選びます。複数ファイルをまとめて追加できます。追加直後は無効で、スイッチで有効にします。",
                ],
                [
                  "03",
                  "いつものように、起動する",
                  "このアプリ、Xbox アプリ、普段のショートカットから起動できます。Mod Studio を開いておく必要はありません。",
                ],
              ].map(([n, t, b]) => (
                <div className="guide-step" key={n}>
                  <span>{n}</span>
                  <div>
                    <h3>{t}</h3>
                    <p>{b}</p>
                  </div>
                </div>
              ))}
              <div className="compatibility">
                <h3>DLL の互換性について</h3>
                <p>
                  対象は Windows x64 の Minecraft GDK です。各 Mod が対象とする
                  Minecraft のバージョンを確認してください。LeviLamina
                  専用プラグインは依存環境が別途必要です。読み込み成功はゲームとの互換性を保証するものではありません。
                </p>
                <p>
                  この版は単体 DLL に対応します。追加データや依存 DLL が必要な
                  Mod は、配布元の構成への対応が必要です。初期化中に終了した DLL
                  は、次の起動でスキップしてログに記録します。ゲーム内でクラッシュする場合は、該当する
                  Mod を無効にしてください。
                </p>
                <p>
                  ゲーム更新後はローダーの状態と互換性を確認してください。変更されたファイルは自動で上書きしません。
                </p>
              </div>
            </section>
          )}
          <footer className="main-footer">
            <span>
              PEXserver <span>/</span> MOD STUDIO
            </span>
            <span>MADE FOR YOUR NEXT ADVENTURE.</span>
          </footer>
        </div>
      </main>
      {remove && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="delete-title"
            onKeyDown={(e) => {
              if (e.key === "Escape") setRemove(undefined);
            }}
          >
            <Trash2 size={26} />
            <h2 id="delete-title">
              {remove.length > 1
                ? `選択した ${remove.length} 件の Mod を削除しますか？`
                : "この Mod を削除しますか？"}
            </h2>
            <p>
              {remove
                .slice(0, 3)
                .map((mod) => mod.name)
                .join("、")}
              {remove.length > 3 ? ` ほか ${remove.length - 3} 件` : ""}{" "}
              をライブラリとゲーム側から削除します。元の DLL は保持されます。
            </p>
            <div className="button-row">
              <button
                className="secondary"
                autoFocus
                onClick={() => setRemove(undefined)}
              >
                キャンセル
              </button>
              <button
                className="primary danger-fill"
                onClick={async () => {
                  const ids = remove.map((mod) => mod.id);
                  setRemove(undefined);
                  if (
                    await action(
                      "remove_mods",
                      { ids },
                      `${ids.length} 件の Mod を削除しました。`,
                    )
                  ) {
                    setSelection(
                      (current) =>
                        new Set([...current].filter((id) => !ids.includes(id))),
                    );
                    selectionAnchor.current = undefined;
                  }
                }}
              >
                削除する
              </button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
