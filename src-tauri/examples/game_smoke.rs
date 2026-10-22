// Explicit, opt-in smoke test against an installed game. Never run with user mods enabled.
// This uses a disposable library, a no-op probe DLL and terminates only the game it launched.
#[allow(dead_code)]
mod smoke {
    include!("../src/manager.rs");
    pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 4 || args[1] != "--run-probe" {
            return Err("Usage: game_smoke --run-probe <Content directory> <probe.dll>".into());
        }
        stopped()?;
        let dir = tempfile::tempdir()?;
        let state = ManagerState::new(dir.path().join("library-data"))?;
        state.library.lock().unwrap().game_path = args[2].clone();
        let g = game(&state.library.lock().unwrap())?;
        if !g.join(MARKER).is_file() {
            install_loader_inner(&state)?;
        }
        if fs::read_dir(g.join("PEXMods"))?
            .filter_map(|e| e.ok())
            .any(|e| e.path().is_dir())
        {
            return Err("Refusing smoke test with an existing deployed Mod directory".into());
        }
        let manifest = fs::read(g.join("PEXMods/active.txt"))?;
        if !manifest.is_empty() {
            return Err("Refusing to replace a non-empty active mod list".into());
        }
        import_mod_inner(args[3].clone(), &state)?;
        let id = state.library.lock().unwrap().mods[0].id.clone();
        set_enabled_inner(id.clone(), true, &state)?;
        let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
            std::process::Command::new(g.join("gamelaunchhelper.exe"))
                .current_dir(&g)
                .spawn()?;
            let mut loaded = false;
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let log = fs::read_to_string(g.join("PEXMods/loader.log")).unwrap_or_default();
                if log
                    .lines()
                    .any(|l| l.starts_with("LOADED ") && l.contains(&id))
                {
                    loaded = true;
                    break;
                }
            }
            if !loaded {
                return Err("Probe DLL was not loaded within 60 seconds".into());
            }
            println!("PASS: probe DLL loaded in actual Minecraft ({id})");
            std::thread::sleep(std::time::Duration::from_secs(15));
            if !running() {
                return Err("Minecraft exited after loading the probe".into());
            }
            println!("PASS: Minecraft remains running after probe initialization");
            Ok(())
        })();
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        for p in sys.processes().values().filter(|p| {
            p.name()
                .to_string_lossy()
                .eq_ignore_ascii_case("Minecraft.Windows.exe")
        }) {
            p.kill();
        }
        for _ in 0..15 {
            if !running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        if running() {
            return Err("Game still running; probe cleanup must wait until it exits".into());
        }
        set_enabled_inner(id.clone(), false, &state)?;
        assert!(fs::read(g.join("PEXMods/active.txt"))?.is_empty());
        remove_mod_inner(id.clone(), &state)?;
        assert!(!g.join("PEXMods").join(id).exists());
        println!("PASS: probe disabled and removed; original empty manifest restored");
        fs::copy(
            g.join("PEXMods/loader.log"),
            Path::new(&args[3]).parent().unwrap().join("game-smoke.log"),
        )?;
        uninstall_loader_inner(&state)?;
        assert!(!g.join("winhttp.dll").exists());
        install_loader_inner(&state)?;
        assert_eq!(fs::read(g.join("PEXMods/PEXModBridge.asi"))?, BRIDGE);
        println!(
            "PASS: loader uninstalled and reinstalled with bundled bridge; no test mods remain"
        );
        result
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    smoke::run()
}
