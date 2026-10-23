use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use sysinfo::{ProcessesToUpdate, System};
use uuid::Uuid;
type Result<T> = std::result::Result<T, String>;
const MARKER: &str = ".pex-mod-studio.json";
const PROXY: &[u8] = include_bytes!("../resources/winhttp.dll");
const BRIDGE: &[u8] = include_bytes!("../resources/PEXModBridge.asi");
const CONFIG: &[u8] = b"[GlobalSets]\r\nLoadPlugins=1\r\nDontLoadFromDllMain=1\r\nLoadFromScriptsOnly=1\r\nLoadRecursively=0\r\nLoadExtraPlugins=PEXMods\\PEXModBridge.asi\r\nDebugLog=1\r\n";
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub enabled: bool,
}
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportOutcome {
    Added,
    Updated,
    Unchanged,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub game_path: String,
    pub mods: Vec<Mod>,
}
#[derive(Serialize, Deserialize)]
struct Owned {
    version: u32,
    files: Vec<(String, String)>,
}
pub struct ManagerState {
    root: PathBuf,
    library: Mutex<Library>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    library: Library,
    installed: bool,
    healthy: bool,
    running: bool,
    detected: bool,
    game_version: String,
    data_path: String,
    logs: String,
    loader_version: &'static str,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    fs::read(path).map_err(err)
}
fn write(path: impl AsRef<Path>, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let path = path.as_ref();
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("Invalid destination")?)
        .map_err(err)?;
    temp.write_all(bytes).map_err(err)?;
    temp.as_file().sync_all().map_err(err)?;
    temp.persist(path).map_err(err)?;
    Ok(())
}
fn pe_x64(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return Err("Windows DLL（PE形式）ではありません。".into());
    }
    let offset = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
    if offset
        .checked_add(24)
        .filter(|end| *end <= bytes.len())
        .is_none()
        || &bytes[offset..offset + 4] != b"PE\0\0"
    {
        return Err("PEヘッダーが壊れています。".into());
    }
    if u16::from_le_bytes(bytes[offset + 4..offset + 6].try_into().unwrap()) != 0x8664 {
        return Err("Minecraft GDK 用の x64 DLL が必要です。".into());
    }
    if u16::from_le_bytes(bytes[offset + 22..offset + 24].try_into().unwrap()) & 0x2000 == 0 {
        return Err("実行ファイルではなく DLL を選択してください。".into());
    }
    Ok(())
}
fn running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        p.name()
            .to_string_lossy()
            .eq_ignore_ascii_case("Minecraft.Windows.exe")
    })
}
fn stopped() -> Result<()> {
    if running() {
        Err("Minecraft を終了してから変更してください。変更は次回起動時に反映されます。".into())
    } else {
        Ok(())
    }
}
fn game(lib: &Library) -> Result<PathBuf> {
    let path = PathBuf::from(&lib.game_path)
        .canonicalize()
        .map_err(|_| "Minecraft の Content フォルダを選択してください。")?;
    if !path.join("Minecraft.Windows.exe").is_file() || !path.join("MicrosoftGame.Config").is_file()
    {
        return Err("Minecraft GDK の Content フォルダではありません。".into());
    }
    Ok(path)
}
fn owned(game: &Path) -> Result<Owned> {
    serde_json::from_slice(&read(game.join(MARKER))?).map_err(err)
}
fn verify_owned(game: &Path, o: &Owned) -> Result<()> {
    validate_owned(o)?;
    for (file, expected) in &o.files {
        if !["winhttp.dll", "winhttp.ini", "PEXMods/PEXModBridge.asi"].contains(&file.as_str()) {
            return Err("管理情報のファイル名が不正です。".into());
        }
        if hash(&read(game.join(file))?) != *expected {
            return Err(format!(
                "{file} が導入後に変更されています。上書き・削除を停止しました。"
            ));
        }
    }
    if o.files.len() != 3 {
        return Err("ローダー管理情報が不完全です。".into());
    }
    Ok(())
}
fn validate_owned(o: &Owned) -> Result<()> {
    let names: std::collections::HashSet<_> =
        o.files.iter().map(|(name, _)| name.as_str()).collect();
    if o.version != 1
        || names.len() != 3
        || !["winhttp.dll", "winhttp.ini", "PEXMods/PEXModBridge.asi"]
            .iter()
            .all(|name| names.contains(name))
    {
        return Err("ローダー管理情報が不正です。".into());
    }
    Ok(())
}
fn validate_library(lib: &Library) -> Result<()> {
    let mut ids = std::collections::HashSet::new();
    for m in &lib.mods {
        if Uuid::parse_str(&m.id).is_err()
            || !ids.insert(&m.id)
            || m.filename.contains(['/', '\\', ':', '\n', '\r'])
            || !m.filename.to_lowercase().ends_with(".dll")
            || m.filename == ".dll"
            || m.sha256.len() != 64
            || !m.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(
                "Mod ライブラリの管理情報が不正です。library.json を確認してください。".into(),
            );
        }
    }
    Ok(())
}
impl ManagerState {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(root.join("library"))?;
        let file = root.join("library.json");
        let library = if file.exists() {
            serde_json::from_slice(&fs::read(file)?).map_err(std::io::Error::other)?
        } else {
            Library {
                game_path: r"C:\XboxGames\Minecraft for Windows\Content".into(),
                mods: vec![],
            }
        };
        validate_library(&library).map_err(std::io::Error::other)?;
        Ok(Self {
            root,
            library: Mutex::new(library),
        })
    }
    fn save(&self, lib: &Library) -> Result<()> {
        write(
            self.root.join("library.json"),
            &serde_json::to_vec_pretty(lib).map_err(err)?,
        )
    }
    fn deploy(&self, lib: &Library) -> Result<()> {
        if !Path::new(&lib.game_path).join(MARKER).exists() {
            return Ok(());
        }
        let g = game(lib)?;
        verify_owned(&g, &owned(&g)?)?;
        let mut manifest = String::new();
        for m in lib.mods.iter().filter(|m| m.enabled) {
            let bytes = read(self.root.join("library").join(&m.id).join(&m.filename))?;
            if hash(&bytes) != m.sha256 {
                return Err(format!("{} の SHA-256 が一致しません。", m.name));
            }
            let dir = g.join("PEXMods").join(&m.id);
            fs::create_dir_all(&dir).map_err(err)?;
            write(dir.join(&m.filename), &bytes)?;
            manifest.push_str(&format!("{}/{}\n", m.id, m.filename));
        }
        write(g.join("PEXMods/active.txt"), manifest.as_bytes())
    }
    fn commit(&self, current: &mut Library, next: Library) -> Result<()> {
        if let Err(e) = self.deploy(&next) {
            return match self.deploy(current) {
                Ok(()) => Err(e),
                Err(restore) => Err(format!("{e} / 元の配置の復元にも失敗しました: {restore}")),
            };
        }
        if let Err(e) = self.save(&next) {
            let _ = self.deploy(current);
            return Err(e);
        }
        *current = next;
        Ok(())
    }
}
#[tauri::command]
pub fn snapshot(state: tauri::State<ManagerState>) -> Result<Snapshot> {
    let lib = state.library.lock().map_err(err)?.clone();
    let g = PathBuf::from(&lib.game_path);
    let installed = g.join(MARKER).exists();
    let healthy = installed && owned(&g).and_then(|o| verify_owned(&g, &o)).is_ok();
    let version = fs::read_to_string(g.join("MicrosoftGame.Config"))
        .ok()
        .and_then(|s| {
            roxmltree::Document::parse(&s).ok().and_then(|d| {
                d.descendants()
                    .find(|n| n.has_tag_name("Identity"))
                    .and_then(|n| n.attribute("Version"))
                    .map(str::to_owned)
            })
        })
        .unwrap_or_else(|| "未検出".into());
    let logs = (|| -> std::io::Result<String> {
        use std::io::{Read, Seek, SeekFrom};
        let mut f = fs::File::open(g.join("PEXMods/loader.log"))?;
        let size = f.metadata()?.len();
        f.seek(SeekFrom::Start(size.saturating_sub(64000)))?;
        let mut b = Vec::new();
        f.read_to_end(&mut b)?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    })()
    .unwrap_or_default();
    Ok(Snapshot {
        detected: game(&lib).is_ok(),
        library: lib,
        installed,
        healthy,
        running: running(),
        game_version: version,
        data_path: state.root.display().to_string(),
        logs,
        loader_version: "Ultimate ASI Loader 10.0.0 + PEX Bridge 0.1.0",
    })
}
#[tauri::command]
pub fn set_game_path(path: String, state: tauri::State<ManagerState>) -> Result<()> {
    stopped()?;
    let mut lib = state.library.lock().map_err(err)?;
    if Path::new(&lib.game_path).join(MARKER).exists() {
        return Err("先に現在のローダーを導入解除してください。".into());
    }
    let mut next = lib.clone();
    next.game_path = path;
    let normalized = game(&next).or_else(|_| {
        next.game_path = Path::new(&next.game_path)
            .join("Content")
            .display()
            .to_string();
        game(&next)
    })?;
    next.game_path = normalized.display().to_string();
    state.save(&next)?;
    *lib = next;
    Ok(())
}
#[tauri::command]
pub fn import_mod(path: String, state: tauri::State<ManagerState>) -> Result<ImportOutcome> {
    import_mod_inner(path, &state)
}
fn import_mod_inner(path: String, state: &ManagerState) -> Result<ImportOutcome> {
    stopped()?;
    let mut lib = state.library.lock().map_err(err)?;
    let p = Path::new(&path);
    if !p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dll")) {
        return Err(".dll ファイルを選択してください。".into());
    }
    let bytes = read(p)?;
    pe_x64(&bytes)?;
    let sha256 = hash(&bytes);
    let filename = p
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .into_owned();
    if filename.contains(['\n', '\r', ':', '/', '\\']) {
        return Err("DLL のファイル名が不正です。".into());
    }
    let matches: Vec<usize> = lib
        .mods
        .iter()
        .enumerate()
        .filter(|(_, m)| m.filename.eq_ignore_ascii_case(&filename))
        .map(|(i, _)| i)
        .collect();
    if matches.len() > 1 {
        return Err(format!(
            "同名の {filename} が複数登録されています。不要な項目を削除してから更新してください。"
        ));
    }
    if let Some(&index) = matches.first() {
        let old = lib.mods[index].clone();
        if old.sha256 == sha256 {
            return Ok(ImportOutcome::Unchanged);
        }
        // Keep the UUID, filename casing, name and enabled state stable across updates.
        let target = state.root.join("library").join(&old.id).join(&old.filename);
        let original = optional_read(&target)?;
        let deployed = if Path::new(&lib.game_path).join(MARKER).exists() {
            let g = game(&lib)?;
            verify_owned(&g, &owned(&g)?)?;
            Some(g.join("PEXMods").join(&old.id).join(&old.filename))
        } else {
            None
        };
        let deployed_original = deployed
            .as_ref()
            .map(|p| optional_read(p))
            .transpose()?
            .flatten();
        let recovery = if deployed.is_some() {
            recovery_edits(Path::new(&lib.game_path), std::slice::from_ref(&old.id))?
        } else {
            vec![]
        };
        let mut next = lib.clone();
        next.mods[index].sha256 = sha256;
        next.mods[index].size = bytes.len() as u64;
        fs::create_dir_all(target.parent().unwrap()).map_err(err)?;
        write(&target, &bytes)?;
        let result = (|| {
            // Refresh an existing disabled deployment too; never create one just for a disabled mod.
            if let Some(p) = &deployed {
                if deployed_original.is_some() {
                    write(p, &bytes)?;
                }
            }
            state.deploy(&next)?;
            apply_recovery(&recovery)?;
            state.save(&next)
        })();
        if let Err(error) = result {
            let mut failures = vec![];
            if let Err(e) = restore_recovery(&recovery) {
                failures.push(e);
            }
            if let Err(e) = restore_file(&target, original.as_deref()) {
                failures.push(e);
            }
            if let Some(p) = &deployed {
                if let Err(e) = restore_file(p, deployed_original.as_deref()) {
                    failures.push(e);
                }
            }
            if let Err(e) = state.deploy(&lib) {
                failures.push(e);
            }
            return Err(if failures.is_empty() {
                error
            } else {
                format!("{error} / 復元エラー: {}", failures.join(" / "))
            });
        }
        *lib = next;
        return Ok(ImportOutcome::Updated);
    }
    if lib.mods.iter().any(|m| m.sha256 == sha256) {
        return Err("同じ内容の DLL が別の名前ですでに追加されています。".into());
    }
    let id = Uuid::new_v4().to_string();
    let dir = state.root.join("library").join(&id);
    fs::create_dir(&dir).map_err(err)?;
    write(dir.join(&filename), &bytes)?;
    let mut next = lib.clone();
    next.mods.push(Mod {
        id,
        name: p.file_stem().unwrap().to_string_lossy().into_owned(),
        filename,
        sha256,
        size: bytes.len() as u64,
        enabled: false,
    });
    if let Err(e) = state.commit(&mut lib, next) {
        let _ = fs::remove_dir_all(dir);
        return Err(e);
    }
    Ok(ImportOutcome::Added)
}
fn optional_read(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}
fn restore_file(path: &Path, bytes: Option<&[u8]>) -> Result<()> {
    if let Some(bytes) = bytes {
        write(path, bytes)
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(err(e)),
        }
    }
}
struct RecoveryEdit {
    path: PathBuf,
    original: Vec<u8>,
    replacement: Vec<u8>,
}
fn recovery_edits(game: &Path, ids: &[String]) -> Result<Vec<RecoveryEdit>> {
    let mut edits = vec![];
    for name in ["blocked.txt", "loading.txt"] {
        let path = game.join("PEXMods").join(name);
        if let Some(original) = optional_read(&path)? {
            let text = String::from_utf8_lossy(&original);
            let matches = |line: &str| ids.iter().any(|id| line.starts_with(&format!("{id}/")));
            if text.lines().any(matches) {
                let replacement = text
                    .lines()
                    .filter(|line| !matches(line))
                    .map(|line| format!("{line}\n"))
                    .collect::<String>()
                    .into_bytes();
                edits.push(RecoveryEdit {
                    path,
                    original,
                    replacement,
                });
            }
        }
    }
    Ok(edits)
}
fn restore_recovery(edits: &[RecoveryEdit]) -> Result<()> {
    let errors: Vec<_> = edits
        .iter()
        .filter_map(|edit| write(&edit.path, &edit.original).err())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" / "))
    }
}
fn apply_recovery(edits: &[RecoveryEdit]) -> Result<()> {
    for edit in edits {
        if let Err(e) = restore_file(
            &edit.path,
            if edit.replacement.is_empty() {
                None
            } else {
                Some(&edit.replacement)
            },
        ) {
            return match restore_recovery(edits) {
                Ok(()) => Err(e),
                Err(r) => Err(format!("{e} / 回復情報の復元失敗: {r}")),
            };
        }
    }
    Ok(())
}
fn selected_mods(lib: &Library, ids: &[String]) -> Result<Vec<Mod>> {
    let ids: std::collections::HashSet<_> = ids.iter().map(String::as_str).collect();
    if ids.is_empty() {
        return Err("Mod を選択してください。".into());
    }
    let mods: Vec<_> = lib
        .mods
        .iter()
        .filter(|m| ids.contains(m.id.as_str()))
        .cloned()
        .collect();
    if mods.len() != ids.len() {
        return Err("選択した Mod が見つかりません。状態を更新して選び直してください。".into());
    }
    Ok(mods)
}
#[tauri::command]
pub fn set_enabled(id: String, enabled: bool, state: tauri::State<ManagerState>) -> Result<()> {
    set_enabled_inner(id, enabled, &state)
}
fn set_enabled_inner(id: String, enabled: bool, state: &ManagerState) -> Result<()> {
    set_mods_enabled_inner(vec![id], enabled, state)
}
#[tauri::command]
pub fn set_mods_enabled(
    ids: Vec<String>,
    enabled: bool,
    state: tauri::State<ManagerState>,
) -> Result<()> {
    set_mods_enabled_inner(ids, enabled, &state)
}
fn set_mods_enabled_inner(ids: Vec<String>, enabled: bool, state: &ManagerState) -> Result<()> {
    stopped()?;
    let mut lib = state.library.lock().map_err(err)?;
    let selected = selected_mods(&lib, &ids)?;
    let mut next = lib.clone();
    for m in &mut next.mods {
        if selected.iter().any(|s| s.id == m.id) {
            m.enabled = enabled;
        }
    }
    let recovery = if enabled && Path::new(&lib.game_path).join(MARKER).exists() {
        let g = game(&lib)?;
        verify_owned(&g, &owned(&g)?)?;
        recovery_edits(&g, &ids)?
    } else {
        vec![]
    };
    apply_recovery(&recovery)?;
    if let Err(e) = state.commit(&mut lib, next) {
        return match restore_recovery(&recovery) {
            Ok(()) => Err(e),
            Err(r) => Err(format!("{e} / 回復情報の復元失敗: {r}")),
        };
    }
    Ok(())
}
#[tauri::command]
pub fn remove_mod(id: String, state: tauri::State<ManagerState>) -> Result<()> {
    remove_mod_inner(id, &state)
}
fn remove_mod_inner(id: String, state: &ManagerState) -> Result<()> {
    remove_mods_inner(vec![id], state)
}
#[tauri::command]
pub fn remove_mods(ids: Vec<String>, state: tauri::State<ManagerState>) -> Result<()> {
    remove_mods_inner(ids, &state)
}
fn remove_mods_inner(ids: Vec<String>, state: &ManagerState) -> Result<()> {
    stopped()?;
    let mut lib = state.library.lock().map_err(err)?;
    let selected = selected_mods(&lib, &ids)?;
    let mut next = lib.clone();
    next.mods.retain(|m| !selected.iter().any(|s| s.id == m.id));
    state.commit(&mut lib, next)?;
    let mut errors = vec![];
    for m in selected {
        let mut dirs = vec![state.root.join("library").join(&m.id)];
        if let Ok(g) = game(&lib) {
            dirs.push(g.join("PEXMods").join(&m.id));
        }
        for dir in dirs {
            if dir.exists() {
                if let Err(e) = restore_file(&dir.join(&m.filename), None) {
                    errors.push(format!("{}: {e}", m.filename));
                }
                // Preserve unknown companion files instead of recursively deleting them.
                let _ = fs::remove_dir(&dir);
            }
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "選択した Mod は解除済みですが、一部のコピーを削除できませんでした: {}",
            errors.join(" / ")
        ));
    }
    Ok(())
}
#[tauri::command]
pub fn install_loader(state: tauri::State<ManagerState>) -> Result<()> {
    install_loader_inner(&state)
}
fn install_loader_inner(state: &ManagerState) -> Result<()> {
    stopped()?;
    let lib = state.library.lock().map_err(err)?;
    let g = game(&lib)?;
    if g.join(MARKER).exists() {
        let o = owned(&g)?;
        validate_owned(&o)?;
        // Restore missing owned files, but never overwrite a changed file.
        for (name, expected) in &o.files {
            let p = g.join(name);
            if p.exists() && hash(&read(&p)?) != *expected {
                return Err(format!("{name} が変更されています。修復を停止しました。"));
            }
        }
        for (name, bytes) in [
            ("winhttp.dll", PROXY),
            ("winhttp.ini", CONFIG),
            ("PEXMods/PEXModBridge.asi", BRIDGE),
        ] {
            let p = g.join(name);
            if !p.exists() {
                fs::create_dir_all(p.parent().unwrap()).map_err(err)?;
                write(p, bytes)?;
            }
        }
        return state.deploy(&lib);
    }
    for name in [
        "winhttp.dll",
        "winhttp.ini",
        "PEXMods",
        "global.ini",
        "scripts/global.ini",
        "plugins/global.ini",
        "update/global.ini",
        "scripts/winhttp.ini",
        "plugins/winhttp.ini",
        "update/winhttp.ini",
    ] {
        if g.join(name).exists() {
            return Err(format!(
                "既存の {name} と衝突します。バックアップ後に手動で移動してください。"
            ));
        }
    }
    let files = [
        ("winhttp.dll", PROXY),
        ("winhttp.ini", CONFIG),
        ("PEXMods/PEXModBridge.asi", BRIDGE),
    ];
    fs::create_dir(g.join("PEXMods")).map_err(err)?;
    let mut created: Vec<PathBuf> = vec![];
    let result = (|| {
        for (name, bytes) in files {
            write(g.join(name), bytes)?;
            created.push(g.join(name));
        }
        let o = Owned {
            version: 1,
            files: files
                .iter()
                .map(|(n, b)| (n.to_string(), hash(b)))
                .collect(),
        };
        write(g.join(MARKER), &serde_json::to_vec_pretty(&o).map_err(err)?)?;
        created.push(g.join(MARKER));
        state.deploy(&lib)
    })();
    if result.is_err() {
        for p in created.iter().rev() {
            let _ = fs::remove_file(p);
        }
        let _ = fs::remove_dir(g.join("PEXMods"));
    }
    result
}
#[tauri::command]
pub fn uninstall_loader(state: tauri::State<ManagerState>) -> Result<()> {
    uninstall_loader_inner(&state)
}
fn uninstall_loader_inner(state: &ManagerState) -> Result<()> {
    stopped()?;
    let lib = state.library.lock().map_err(err)?;
    let g = game(&lib)?;
    let o = owned(&g)?;
    validate_owned(&o)?;
    for (name, expected) in &o.files {
        let p = g.join(name);
        if p.exists() && hash(&read(&p)?) != *expected {
            return Err(format!(
                "{name} が変更されています。導入解除を停止しました。"
            ));
        }
    }
    for (name, _) in &o.files {
        let p = g.join(name);
        if p.exists() {
            fs::remove_file(p).map_err(err)?;
        }
    }
    for m in &lib.mods {
        let dir = g.join("PEXMods").join(&m.id);
        let file = dir.join(&m.filename);
        if file.is_file() {
            fs::remove_file(file).map_err(err)?;
        }
        if dir.is_dir() {
            fs::remove_dir(dir).map_err(err)?;
        }
    }
    for name in ["active.txt", "loading.txt", "blocked.txt", "loader.log"] {
        let p = g.join("PEXMods").join(name);
        if p.exists() {
            fs::remove_file(p).map_err(err)?;
        }
    }
    fs::remove_file(g.join(MARKER)).map_err(err)?;
    // Unknown companion files are retained, never recursively deleted.
    let _ = fs::remove_dir(g.join("PEXMods"));
    Ok(())
}
#[tauri::command]
pub fn launch_game(state: tauri::State<ManagerState>) -> Result<()> {
    stopped()?;
    let lib = state.library.lock().map_err(err)?;
    let g = game(&lib)?;
    if g.join(MARKER).exists() {
        state.deploy(&lib)?;
    }
    let helper = g.join("gamelaunchhelper.exe");
    if !helper.is_file() {
        return Err(
            "gamelaunchhelper.exe が見つかりません。Xbox アプリで修復してください。".into(),
        );
    }
    std::process::Command::new(helper)
        .current_dir(g)
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
pub fn open_folder(kind: String, state: tauri::State<ManagerState>) -> Result<()> {
    let lib = state.library.lock().map_err(err)?;
    let p = match kind.as_str() {
        "library" => state.root.join("library"),
        "game" => game(&lib)?,
        _ => return Err("Unknown folder".into()),
    };
    std::process::Command::new("explorer.exe")
        .arg(p)
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_pe() {
        assert!(pe_x64(b"MZ").is_err());
        let mut b = vec![0; 100];
        b[..2].copy_from_slice(b"MZ");
        b[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(pe_x64(&b).is_err());
    }
    #[test]
    fn checks_architecture_and_dll_flag() {
        let mut b = vec![0; 100];
        b[..2].copy_from_slice(b"MZ");
        b[60..64].copy_from_slice(&64u32.to_le_bytes());
        b[64..68].copy_from_slice(b"PE\0\0");
        b[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        assert!(pe_x64(&b).is_err());
        b[86..88].copy_from_slice(&0x2000u16.to_le_bytes());
        assert!(pe_x64(&b).is_ok());
        b[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
        assert!(pe_x64(&b).is_err());
    }
    #[test]
    fn atomic_replace() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("file");
        write(&p, b"old").unwrap();
        write(&p, b"new").unwrap();
        assert_eq!(read(&p).unwrap(), b"new");
    }
    #[test]
    fn marker_cannot_delete_arbitrary_files() {
        let d = tempfile::tempdir().unwrap();
        assert!(verify_owned(
            d.path(),
            &Owned {
                version: 1,
                files: vec![("../other.dll".into(), "".into())]
            }
        )
        .is_err());
    }
    fn fixture() -> (tempfile::TempDir, ManagerState, PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let g = d.path().join("game");
        fs::create_dir(&g).unwrap();
        fs::write(g.join("Minecraft.Windows.exe"), b"fixture").unwrap();
        fs::write(
            g.join("MicrosoftGame.Config"),
            b"<Game><Identity Version='test'/></Game>",
        )
        .unwrap();
        let s = ManagerState::new(d.path().join("app")).unwrap();
        s.library.lock().unwrap().game_path = g.display().to_string();
        (d, s, g)
    }
    #[test]
    fn full_lifecycle_and_missing_file_repair() {
        let (d, s, g) = fixture();
        let source = d.path().join("probe.dll");
        fs::write(&source, BRIDGE).unwrap();
        install_loader_inner(&s).unwrap();
        assert!(g.join("winhttp.dll").is_file());
        import_mod_inner(source.display().to_string(), &s).unwrap();
        assert_eq!(
            import_mod_inner(source.display().to_string(), &s).unwrap(),
            ImportOutcome::Unchanged
        );
        let id = s.library.lock().unwrap().mods[0].id.clone();
        assert!(fs::read_to_string(g.join("PEXMods/active.txt"))
            .unwrap()
            .is_empty());
        set_enabled_inner(id.clone(), true, &s).unwrap();
        assert!(fs::read_to_string(g.join("PEXMods/active.txt"))
            .unwrap()
            .contains(&id));
        assert!(g.join("PEXMods").join(&id).join("probe.dll").is_file());
        set_enabled_inner(id.clone(), false, &s).unwrap();
        assert!(fs::read_to_string(g.join("PEXMods/active.txt"))
            .unwrap()
            .is_empty());
        fs::remove_file(g.join("winhttp.dll")).unwrap();
        install_loader_inner(&s).unwrap();
        assert_eq!(read(g.join("winhttp.dll")).unwrap(), PROXY);
        remove_mod_inner(id, &s).unwrap();
        assert!(s.library.lock().unwrap().mods.is_empty());
        assert!(source.exists());
        uninstall_loader_inner(&s).unwrap();
        assert!(!g.join(MARKER).exists());
        assert!(!g.join("winhttp.dll").exists());
        assert!(!g.join("PEXMods").exists());
    }
    #[test]
    fn preserves_existing_and_changed_loader() {
        let (_d, s, g) = fixture();
        fs::write(g.join("winhttp.dll"), b"existing").unwrap();
        assert!(install_loader_inner(&s).is_err());
        assert_eq!(read(g.join("winhttp.dll")).unwrap(), b"existing");
        fs::remove_file(g.join("winhttp.dll")).unwrap();
        install_loader_inner(&s).unwrap();
        fs::write(g.join("winhttp.dll"), b"changed").unwrap();
        assert!(uninstall_loader_inner(&s).is_err());
        assert!(install_loader_inner(&s).is_err());
        assert_eq!(read(g.join("winhttp.dll")).unwrap(), b"changed");
    }
    #[test]
    fn rejects_library_traversal() {
        let lib = Library {
            game_path: String::new(),
            mods: vec![Mod {
                id: "../escape".into(),
                name: "bad".into(),
                filename: "bad.dll".into(),
                sha256: "0".repeat(64),
                size: 0,
                enabled: false,
            }],
        };
        assert!(validate_library(&lib).is_err());
    }
    #[test]
    fn same_filename_update_preserves_identity_and_enabled_state() {
        let (d, s, g) = fixture();
        install_loader_inner(&s).unwrap();
        let source = d.path().join("Example.dll");
        fs::write(&source, BRIDGE).unwrap();
        assert_eq!(
            import_mod_inner(source.display().to_string(), &s).unwrap(),
            ImportOutcome::Added
        );
        let original = s.library.lock().unwrap().mods[0].clone();
        set_enabled_inner(original.id.clone(), true, &s).unwrap();
        let mut newer = BRIDGE.to_vec();
        newer.extend_from_slice(b"new version");
        let updated = d.path().join("EXAMPLE.DLL");
        fs::write(&updated, &newer).unwrap();
        assert_eq!(
            import_mod_inner(updated.display().to_string(), &s).unwrap(),
            ImportOutcome::Updated
        );
        let lib = s.library.lock().unwrap();
        assert_eq!(lib.mods.len(), 1);
        let m = &lib.mods[0];
        assert_eq!(m.id, original.id);
        assert!(m.enabled);
        assert_eq!(m.filename, "Example.dll");
        assert_eq!(m.sha256, hash(&newer));
        assert_eq!(m.size, newer.len() as u64);
        assert_eq!(
            read(s.root.join("library").join(&m.id).join(&m.filename)).unwrap(),
            newer
        );
        assert_eq!(
            read(g.join("PEXMods").join(&m.id).join(&m.filename)).unwrap(),
            newer
        );
    }
    #[test]
    fn disabled_update_stays_disabled_and_identical_import_is_a_noop() {
        let (d, s, g) = fixture();
        install_loader_inner(&s).unwrap();
        let source = d.path().join("mod.dll");
        fs::write(&source, BRIDGE).unwrap();
        import_mod_inner(source.display().to_string(), &s).unwrap();
        let id = s.library.lock().unwrap().mods[0].id.clone();
        set_enabled_inner(id.clone(), true, &s).unwrap();
        set_enabled_inner(id.clone(), false, &s).unwrap();
        let mut newer = BRIDGE.to_vec();
        newer.push(1);
        fs::write(&source, &newer).unwrap();
        assert_eq!(
            import_mod_inner(source.display().to_string(), &s).unwrap(),
            ImportOutcome::Updated
        );
        assert!(!s.library.lock().unwrap().mods[0].enabled);
        assert!(read(g.join("PEXMods/active.txt")).unwrap().is_empty());
        assert_eq!(
            read(g.join("PEXMods").join(&id).join("mod.dll")).unwrap(),
            newer
        );
        assert_eq!(
            import_mod_inner(source.display().to_string(), &s).unwrap(),
            ImportOutcome::Unchanged
        );
    }
    #[test]
    fn failed_update_restores_library_and_deployed_bytes() {
        let (d, s, g) = fixture();
        install_loader_inner(&s).unwrap();
        let source = d.path().join("mod.dll");
        fs::write(&source, BRIDGE).unwrap();
        import_mod_inner(source.display().to_string(), &s).unwrap();
        let id = s.library.lock().unwrap().mods[0].id.clone();
        set_enabled_inner(id.clone(), true, &s).unwrap();
        fs::remove_file(s.root.join("library.json")).unwrap();
        fs::create_dir(s.root.join("library.json")).unwrap();
        let mut newer = BRIDGE.to_vec();
        newer.push(2);
        fs::write(&source, newer).unwrap();
        assert!(import_mod_inner(source.display().to_string(), &s).is_err());
        assert_eq!(
            read(s.root.join("library").join(&id).join("mod.dll")).unwrap(),
            BRIDGE
        );
        assert_eq!(
            read(g.join("PEXMods").join(&id).join("mod.dll")).unwrap(),
            BRIDGE
        );
        assert_eq!(s.library.lock().unwrap().mods[0].sha256, hash(BRIDGE));
    }
    #[test]
    fn invalid_update_does_not_replace_existing_mod() {
        let (d, s, _g) = fixture();
        let source = d.path().join("mod.dll");
        fs::write(&source, BRIDGE).unwrap();
        import_mod_inner(source.display().to_string(), &s).unwrap();
        fs::write(&source, b"invalid").unwrap();
        assert!(import_mod_inner(source.display().to_string(), &s).is_err());
        let lib = s.library.lock().unwrap();
        assert_eq!(lib.mods.len(), 1);
        assert_eq!(lib.mods[0].sha256, hash(BRIDGE));
    }
    #[test]
    fn updated_and_reenabled_mods_clear_only_their_recovery_records() {
        let (d, s, g) = fixture();
        install_loader_inner(&s).unwrap();
        let source = d.path().join("mod.dll");
        fs::write(&source, BRIDGE).unwrap();
        import_mod_inner(source.display().to_string(), &s).unwrap();
        let id = s.library.lock().unwrap().mods[0].id.clone();
        let block = format!("{id}/mod.dll\nother/mod.dll\n");
        let loading = format!("{id}/mod.dll");
        fs::write(g.join("PEXMods/blocked.txt"), &block).unwrap();
        fs::write(g.join("PEXMods/loading.txt"), &loading).unwrap();
        let mut newer = BRIDGE.to_vec();
        newer.push(5);
        fs::write(&source, &newer).unwrap();
        import_mod_inner(source.display().to_string(), &s).unwrap();
        assert_eq!(
            fs::read_to_string(g.join("PEXMods/blocked.txt")).unwrap(),
            "other/mod.dll\n"
        );
        assert!(!g.join("PEXMods/loading.txt").exists());
        fs::write(g.join("PEXMods/blocked.txt"), &block).unwrap();
        fs::write(g.join("PEXMods/loading.txt"), &loading).unwrap();
        set_enabled_inner(id.clone(), true, &s).unwrap();
        assert_eq!(
            fs::read_to_string(g.join("PEXMods/blocked.txt")).unwrap(),
            "other/mod.dll\n"
        );
        assert!(!g.join("PEXMods/loading.txt").exists());
        fs::write(g.join("PEXMods/blocked.txt"), &block).unwrap();
        fs::write(g.join("PEXMods/loading.txt"), &loading).unwrap();
        fs::remove_file(s.root.join("library.json")).unwrap();
        fs::create_dir(s.root.join("library.json")).unwrap();
        newer.push(6);
        fs::write(&source, newer).unwrap();
        assert!(import_mod_inner(source.display().to_string(), &s).is_err());
        assert_eq!(
            fs::read_to_string(g.join("PEXMods/blocked.txt")).unwrap(),
            block
        );
        assert_eq!(
            fs::read_to_string(g.join("PEXMods/loading.txt")).unwrap(),
            loading
        );
    }
    #[test]
    fn bulk_changes_validate_ids_and_affect_selected_items_only() {
        let (d, s, g) = fixture();
        install_loader_inner(&s).unwrap();
        for i in 0..3 {
            let source = d.path().join(format!("mod{i}.dll"));
            let mut bytes = BRIDGE.to_vec();
            bytes.push(i);
            fs::write(&source, bytes).unwrap();
            import_mod_inner(source.display().to_string(), &s).unwrap();
        }
        let ids: Vec<_> = s
            .library
            .lock()
            .unwrap()
            .mods
            .iter()
            .map(|m| m.id.clone())
            .collect();
        assert!(set_mods_enabled_inner(vec![ids[0].clone(), "missing".into()], true, &s).is_err());
        assert!(s.library.lock().unwrap().mods.iter().all(|m| !m.enabled));
        set_mods_enabled_inner(ids[..2].to_vec(), true, &s).unwrap();
        {
            let lib = s.library.lock().unwrap();
            assert!(lib.mods[0].enabled && lib.mods[1].enabled && !lib.mods[2].enabled);
        }
        assert_eq!(
            fs::read_to_string(g.join("PEXMods/active.txt"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        set_mods_enabled_inner(ids[..2].to_vec(), false, &s).unwrap();
        assert!(read(g.join("PEXMods/active.txt")).unwrap().is_empty());
        assert!(remove_mods_inner(vec![ids[0].clone(), "missing".into()], &s).is_err());
        assert_eq!(s.library.lock().unwrap().mods.len(), 3);
        remove_mods_inner(ids[..2].to_vec(), &s).unwrap();
        let lib = s.library.lock().unwrap();
        assert_eq!(lib.mods.len(), 1);
        assert_eq!(lib.mods[0].id, ids[2]);
        for id in &ids[..2] {
            assert!(!s.root.join("library").join(id).exists());
            assert!(!g.join("PEXMods").join(id).exists());
        }
        assert!(d.path().join("mod0.dll").exists());
    }
}
