use fwatch::{BasicTarget, Transition, WatchState, Watcher};
use libsts::Save;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ststracker_base::UpdateRequest;
use std::collections::HashMap;

fn app() -> Result<(), &'static str> {
    // load the settings file
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name("Settings")).unwrap();

    let mut settings = settings
        .try_into::<HashMap<String, String>>()
        .expect("Failed to load settings");

    // Check that the settings file contains all the settings we require
    let backend_api_key = settings
        .remove("backend_api_key")
        .expect("Missing setting `backend_api_key`");

    let installation_directory = settings
        .remove("sts_directory")
        .expect("Missing setting `sts_directory`");

    let server_addr = settings
        .remove("server_addr")
        .expect("Missing setting `server_addr`");

    // First, check that the installation folder exists
    let installation_path = Path::new(&installation_directory);

    if !installation_path.exists() {
        return Err("installation directory does not exist");
    }

    // Check that the update delay makes sense
    let duration = Duration::from_secs(5);

    // Initialize our file watcher
    let mut watcher = initialize_watcher(installation_path)?;
    let mut save = None;

    loop {
        // check for any changes in states
        for (index, transition) in watcher.watch().iter().enumerate() {
            // Does this save file exist?
            if let Some(WatchState::Exists(_)) = watcher.get_state(index) {
                let path = watcher.get_path(index).unwrap();

                // Do we need to reparse our save file?
                let need_update = save.is_none()
                    || match transition {
                        Transition::Created => true,
                        Transition::Modified => true,
                        _ => false,
                    };

                if need_update {
                    // Should be unnecessary, but check again
                    if path.exists() {
                        let contents = std::fs::read_to_string(path)
                            .map_err(|_e| "could not open save file")?;
                        let current_save =
                            Save::new(&contents).map_err(|_e| "could not parse save file")?;

                        // send details of our current save to the server
                        if update_server(&server_addr, &current_save, &backend_api_key).is_err() {
                            println!("failed to update server");
                        }

                        save = Some(current_save);
                    }
                }
            }
        }

        std::thread::sleep(duration);
    }
}

fn update_server(server_addr : &str, _save: &Save, backend_api_key: &String) -> Result<(), &'static str> {
    let request = reqwest::Client::new()
        .post(server_addr)
        .json(&UpdateRequest::new(_save, backend_api_key.clone()))
        .send();

    if request.is_err() {
        Err("Failed to update server")
    } else {
        Ok(())
    }
}

fn initialize_watcher<T: AsRef<Path>>(
    installation_path: T,
) -> Result<Watcher<BasicTarget>, &'static str> {
    // Check that the save folder exists
    let mut save_path = PathBuf::from(installation_path.as_ref());
    save_path.push("saves");

    if !save_path.exists() {
        return Err("save folder not found");
    }

    let mut watcher = Watcher::new();

    // Currently there are only three characters, but later on we could expand on this
    // by either modifying this vector, or by changing this function to watch any files
    // with extension .autosave
    let savefile_names = vec![
        "IRONCLAD.autosave",
        "DEFECT.autosave",
        "THE_SILENT.autosave",
    ];

    for filename in &savefile_names {
        let mut file_path = PathBuf::from(&save_path);
        file_path.push(*filename);

        watcher.add_target(BasicTarget::new(file_path));
    }

    Ok(watcher)
}

fn main() {
    std::process::exit(match app() {
        Ok(_) => 0,
        Err(err) => {
            eprintln!("error: {}", err);
            eprintln!("run ./stswatcher -h for help");
            1
        }
    });
}
